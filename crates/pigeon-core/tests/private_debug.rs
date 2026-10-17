//! Diagnostic contracts for authorities and buffered protocol bytes (ADR 0022).
//! Formatting must not change the actual session or roster authority.

use pigeon_core::filter::{Action, Config, Filter, Role, Rule};
use pigeon_core::roster::Roster;

fn config() -> Config {
    Config {
        role: Role::Custodian,
        auth_token: None,
        peer_uuid: Some("peer-uuid".into()),
        peer_custodian: false,
    }
}

#[test]
fn filter_configuration_does_not_format_the_annex_token() {
    let mut config = config();
    config.auth_token = Some("synthetic-filter-authority".into());
    for text in [format!("{config:?}"), format!("{config:#?}")] {
        assert!(
            !text.contains("synthetic-filter"),
            "filter authority appeared in Debug"
        );
        assert!(text.contains("Custodian"));
        assert!(text.contains("peer-uuid"));
    }
    assert!(
        config
            .auth_token
            .as_ref()
            .is_some_and(|t| t == "synthetic-filter-authority")
    );
}

#[test]
fn incomplete_protocol_bytes_do_not_appear_in_filter_diagnostics() {
    let private = b"AUTH synthetic-private-partial-line";
    let mut filter = Filter::new(config(), false);
    assert!(filter.from_client(private).is_empty());
    let raw = format!("{private:?}");
    for text in [format!("{filter:?}"), format!("{filter:#?}")] {
        assert!(
            !text.contains(&raw),
            "buffered authentication bytes appeared in Debug"
        );
        assert!(
            !text.contains("65,\n"),
            "alternate Debug disclosed raw protocol bytes"
        );
        assert!(text.contains("Filter"));
    }
}

#[test]
fn protocol_action_diagnostics_do_not_format_raw_stream_or_refused_bytes() {
    let private = b"AUTH synthetic-private-action".to_vec();
    let raw = format!("{private:?}");
    for action in [
        Action::ToServer(private.clone()),
        Action::ToClient(private.clone()),
        Action::ContentToServer(private.clone()),
        Action::ContentToClient(private.clone()),
        Action::Refused {
            rule: Rule::Push,
            line: private.clone(),
        },
    ] {
        let text = format!("{action:?}");
        assert!(!text.contains(&raw), "action bytes appeared in Debug");
        assert!(text.contains(&private.len().to_string()));
    }
}

#[test]
fn a_rosters_invitation_is_hidden_by_nested_debug() {
    let roster = Roster {
        rules: 1,
        group: "diagnostic-group".into(),
        invitation: "fedcba9876543210".repeat(4),
        k: 2,
        retention_days: 30,
        relay: None,
        telemetry_device: None,
        persons: Default::default(),
        devices: Default::default(),
        removed: Default::default(),
        folders: Vec::new(),
    };
    let text = format!("{:?}", vec![roster.clone()]);
    assert!(
        !text.contains("fedcba9876543210"),
        "roster invitation appeared in Debug"
    );
    assert!(text.contains("diagnostic-group"));
    assert_eq!(roster.invitation.len(), 64);
}
