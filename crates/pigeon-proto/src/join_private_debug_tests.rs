//! Invitation diagnostics redact authority while the intended wire codec
//! still delivers the complete invitation to its recipient (ADR 0022).

use super::{INVITATION_VERSION, Invitation};

#[test]
fn invitation_debug_redacts_the_join_secret_without_changing_the_wire_codec() {
    let invitation = Invitation {
        v: INVITATION_VERSION,
        group: "diagnostic-group".into(),
        name: "fixture-group".into(),
        k: 2,
        admitters: Vec::new(),
        secret: "synthetic-invitation-authority".into(),
        relay: None,
        refounded: None,
    };
    for diagnostic in [format!("{invitation:?}"), format!("{invitation:#?}")] {
        assert!(diagnostic.contains("diagnostic-group"));
        assert!(diagnostic.contains("fixture-group"));
        assert!(
            !diagnostic.contains("synthetic-invitation"),
            "invitation authority appeared in Debug"
        );
    }
    let restored = super::decode(&super::encode(&invitation)).expect("valid invitation wire codec");
    assert!(
        restored == invitation,
        "diagnostic redaction changed the wire value"
    );
}
