//! Installation diagnostics (10 §10.7). The transport helper must run
//! from the same environment git-annex uses and match this release.
//! Its version probe does not require a daemon socket or a group.

use pigeon_git::GitEnv;
use std::time::Duration;

pub(crate) async fn helper(env: &GitEnv) -> Result<(), String> {
    let mut command = tokio::process::Command::new("git-annex-p2p-iroh");
    command.arg("--version").kill_on_drop(true);
    env.apply(&mut command);
    let output = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .map_err(|_| "transport helper version probe timed out".to_owned())?
        .map_err(|e| e.to_string())?;
    let expected = format!("git-annex-p2p-iroh {}", env!("CARGO_PKG_VERSION"));
    if output.status.success() && output.stdout == format!("{expected}\n").as_bytes() {
        Ok(())
    } else {
        Err(format!("transport helper does not match {expected}"))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "isolated executable fixtures")]
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn missing_stale_and_matching_helpers_are_distinguished() {
        let dir = tempfile::tempdir().unwrap();
        let env = GitEnv {
            extra: vec![("PATH".into(), dir.path().as_os_str().into())],
            ..Default::default()
        };
        assert!(helper(&env).await.is_err());
        let path = dir.path().join("git-annex-p2p-iroh");
        for (version, works) in [("old", false), (env!("CARGO_PKG_VERSION"), true)] {
            std::fs::write(
                &path,
                format!("#!/bin/sh\nprintf 'git-annex-p2p-iroh {version}\\n'\n"),
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
            assert_eq!(helper(&env).await.is_ok(), works);
        }
        std::fs::write(&path, "#!/bin/sh\nexit 1\n").unwrap();
        assert!(helper(&env).await.is_err());
    }
}
