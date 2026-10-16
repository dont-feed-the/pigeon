//! Existing shell startup hooks must load working completions after an update,
//! without a daemon or a warning. Both entry points use the same live protocol.
#![allow(clippy::unwrap_used, clippy::expect_used, reason = "tests")]

use std::path::Path;
use std::process::Command;

fn pigeon(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_pigeon"));
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("PIGEON_SOCKET", home.join("no-daemon.sock"))
        .env_remove("COMPLETE");
    cmd
}

#[test]
fn existing_hooks_print_the_current_registration_without_warnings() {
    let home = tempfile::tempdir().unwrap();
    for shell in ["bash", "zsh", "fish", "elvish", "powershell"] {
        let old = pigeon(home.path())
            .args(["completions", shell])
            .output()
            .unwrap();
        assert!(
            old.status.success() && old.stderr.is_empty(),
            "{shell}: {}",
            String::from_utf8_lossy(&old.stderr)
        );
        let current = pigeon(home.path()).env("COMPLETE", shell).output().unwrap();
        assert!(current.status.success() && current.stderr.is_empty());
        assert!(!old.stdout.is_empty());
        assert_eq!(old.stdout, current.stdout, "{shell}");
    }
    for args in [&["completions"][..], &["completions", "unknown-shell"]] {
        let out = pigeon(home.path()).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
    }
}

#[test]
fn an_old_bash_startup_hook_completes_commands_without_a_daemon() {
    let home = tempfile::tempdir().unwrap();
    let bin = Path::new(env!("CARGO_BIN_EXE_pigeon"));
    let path = std::env::join_paths(std::iter::once(bin.parent().unwrap().to_path_buf()).chain(
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
    ))
    .unwrap();
    // The startup line is the one written by pigeon 0.9's setup command.
    let out = Command::new("bash")
        .args(["--noprofile", "--norc", "-c"])
        .arg(
            r#"
if command -v pigeon >/dev/null; then eval "$(pigeon completions bash)"; fi
COMP_WORDS=(pigeon sta)
COMP_CWORD=1
COMP_TYPE=9
_clap_complete_pigeon pigeon sta ''
printf '%s\n' "${COMPREPLY[@]}"
"#,
        )
        .env("PATH", path)
        .env("HOME", home.path())
        .env("XDG_CONFIG_HOME", home.path().join("config"))
        .env("PIGEON_SOCKET", home.path().join("no-daemon.sock"))
        .env_remove("COMPLETE")
        .output()
        .unwrap();
    assert!(
        out.status.success() && out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(String::from_utf8(out.stdout).unwrap().trim(), "status");
}
