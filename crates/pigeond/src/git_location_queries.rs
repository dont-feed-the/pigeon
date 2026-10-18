//! Location-log reads omit unused URLs when the native command advertises it.
//! Stock builds use the original query. Both modes return claims, not proofs
//! of physical content, and keep the caller's input, deadline and exit status.

use std::time::Duration;

use super::{Error, Git};

impl Git {
    /// Read complete JSON location claims without requesting download URLs.
    /// A failed capability probe safely selects the ordinary query for this
    /// repository's lifetime. Concurrent and download views share one probe.
    pub(crate) async fn location_claims(
        &self,
        input: &[u8],
        timeout: Duration,
    ) -> Result<(Option<i32>, pigeon_git::Output), Error> {
        let no_urls = self
            .whereis_no_urls
            .get_or_init(|| async {
                self.repo
                    .run(
                        &["annex", "whereis", "--help"],
                        None,
                        Duration::from_secs(5),
                    )
                    .await
                    .is_ok_and(|out| {
                        out.text()
                            .split_ascii_whitespace()
                            .any(|s| s == "--no-urls")
                    })
            })
            .await;
        let mut args = vec!["annex", "whereis", "--batch-keys", "--json"];
        if *no_urls {
            args.push("--no-urls");
        }
        self.repo.run_status(&args, Some(input), timeout).await
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        reason = "isolated native capability pipe fixtures"
    )]

    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[tokio::test]
    async fn only_successfully_advertised_capabilities_change_queries() {
        for (help, status, supported) in [
            ("  --no-urls  omit URLs", 0, true),
            ("  --json  JSON output", 0, false),
            ("  --no-urls-extra  unrelated option", 0, false),
            ("  --no-urls  omit URLs", 1, false),
        ] {
            let root = tempfile::tempdir().unwrap();
            let script = root.path().join("git");
            std::fs::write(
                &script,
                r#"#!/usr/bin/env python3
import json, os, sys
if '--help' in sys.argv:
    with open('probes', 'a') as f: f.write('probe\n')
    print(os.environ['FIXTURE_HELP'])
    sys.exit(int(os.environ['FIXTURE_STATUS']))
assert ('--no-urls' in sys.argv) == (os.environ['FIXTURE_SUPPORTED'] == 'true')
assert all(arg in sys.argv for arg in ['annex', 'whereis', '--batch-keys', '--json'])
for key in sys.stdin.read().splitlines():
    print(json.dumps(dict(key=key, untrusted=[dict(uuid='claim-only')], success=False)))
sys.exit(1)
"#,
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
            let git = Git::new(
                root.path(),
                pigeon_git::GitEnv {
                    path_first: vec![root.path().into()],
                    extra: vec![
                        ("FIXTURE_HELP".into(), help.into()),
                        ("FIXTURE_STATUS".into(), status.to_string().into()),
                        ("FIXTURE_SUPPORTED".into(), supported.to_string().into()),
                    ],
                },
            );
            let clone = git.clone();
            let download = git.for_download();
            let input = b"synthetic-key\n";
            let timeout = Duration::from_secs(5);
            let replies = tokio::join!(
                git.location_claims(input, timeout),
                clone.location_claims(input, timeout),
                download.location_claims(input, timeout),
            );
            for reply in [replies.0, replies.1, replies.2] {
                let (status, out) = reply.unwrap();
                assert_eq!(status, Some(1));
                let row: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
                assert_eq!(row["key"], "synthetic-key");
                assert_eq!(row["untrusted"][0]["uuid"], "claim-only");
                assert_eq!(row["success"], false);
            }
            assert_eq!(
                std::fs::read_to_string(root.path().join("probes")).unwrap(),
                "probe\n"
            );
        }
    }
}
