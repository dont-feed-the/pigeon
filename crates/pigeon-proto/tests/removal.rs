//! What removing a device does, as the question before it says (09 §9.7):
//! a backup machine counts the backup machines left against K.

use pigeon_proto::Response;
use pigeon_proto::api::{Machine, Removal};

fn machine(name: &str, custodian: bool, recovery: bool) -> Machine {
    Machine {
        name: name.into(),
        device: name.into(),
        custodian,
        recovery,
        heard_at: None,
        this_device: false,
    }
}

#[test]
fn a_removal_counts_the_backup_machines_left() {
    let answers = [Response::Machines {
        machines: vec![
            machine("lap", false, false),
            machine("nas", true, false),
            machine("box", true, false),
            machine("sheet", false, true),
        ],
        copies: Some(2),
    }];
    let backup = Removal::Backup { left: 1, needed: 2 };
    assert_eq!(Removal::of(&answers, "nas"), backup);
    assert_eq!(Removal::of(&answers, "lap"), Removal::Computer);
    assert_eq!(Removal::of(&answers, "sheet"), Removal::Sheet);
    assert_eq!(Removal::of(&[Response::Done], "nas"), Removal::Computer);
    // An older daemon does not tell K: the last backup machine still warns.
    let older = [Response::Machines {
        machines: vec![machine("nas", true, false)],
        copies: None,
    }];
    let last = Removal::Backup { left: 0, needed: 1 };
    assert_eq!(Removal::of(&older, "nas"), last);
}
