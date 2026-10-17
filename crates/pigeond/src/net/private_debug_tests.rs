//! Secret-bearing network objects must retain useful diagnostic identity
//! without formatting keys or annex authentication material (ADR 0022).

use super::{NetConfig, Reach, Repository};

#[test]
fn debug_of_network_configuration_does_not_format_the_signing_key() {
    let config = NetConfig {
        secret_key: [173; 32],
        bind: "127.0.0.1:0".parse().expect("fixture address"),
        mdns: false,
        reach: Reach::Local,
    };
    for diagnostic in [format!("{config:?}"), format!("{config:#?}")] {
        assert!(diagnostic.contains("NetConfig"));
        assert!(diagnostic.contains("127.0.0.1"));
        assert!(!diagnostic.contains("173"), "signing key appeared in Debug");
    }
    assert_eq!(config.secret_key, [173; 32]);
}

#[test]
fn nested_repository_diagnostics_do_not_format_annex_credentials() {
    let repository = Repository {
        group: "diagnostic-group".into(),
        git_dir: "/fixture/.git".into(),
        worktree: Some("/fixture".into()),
        role: pigeon_core::filter::Role::Laptop,
        auth_token: Some("synthetic-annex-authentication-material".into()),
        checking: false,
    };
    for diagnostic in [
        format!("{:?}", vec![repository.clone()]),
        format!("{repository:#?}"),
    ] {
        assert!(diagnostic.contains("diagnostic-group"));
        assert!(diagnostic.contains("Laptop"));
        assert!(
            !diagnostic.contains("synthetic-annex"),
            "annex credential appeared in Debug"
        );
    }
    assert_eq!(
        repository.auth_token.as_deref(),
        Some("synthetic-annex-authentication-material")
    );
}
