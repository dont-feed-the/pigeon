//! Real annex mode correction must neither clean working bytes nor replace pointers.
#![allow(clippy::unwrap_used, reason = "isolated native index-mode fixture")]

use super::*;
use std::os::unix::fs::PermissionsExt;

struct Fixture {
    temp: tempfile::TempDir,
    group: Group,
    paths: Vec<String>,
    trace: std::path::PathBuf,
}

async fn fixture() -> Option<Fixture> {
    let temp = tempfile::tempdir().unwrap();
    let git = crate::git::Git::new(temp.path(), pigeon_git::GitEnv::default());
    if git.run(&["annex", "version", "--raw"]).await.is_err() {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return None;
    }
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Mode fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "mode@example.test"])
        .await
        .unwrap();
    git.run(&["annex", "init", "mode"]).await.unwrap();
    let trace = temp.path().join("mode-trace.json");
    let env = pigeon_git::GitEnv {
        extra: vec![("GIT_TRACE2_EVENT".into(), trace.clone().into_os_string())],
        ..Default::default()
    };
    let group = Group::open(&"a".repeat(40), temp.path(), &env)
        .await
        .unwrap();
    crate::settings::check(&group).await;
    let paths: Vec<String> = [
        "ordinary.bin",
        "executable.sh",
        "--option.bin",
        "tab\tname.bin",
        "line\nname.bin",
        ":(glob)*",
        "z.bin",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    for (n, path) in paths.iter().enumerate() {
        let file = temp.path().join(path);
        std::fs::write(&file, format!("original bytes {n}\n")).unwrap();
        std::fs::set_permissions(
            file,
            std::fs::Permissions::from_mode(if n % 2 == 0 { 0o755 } else { 0o644 }),
        )
        .unwrap();
    }
    let input: Vec<_> = paths.iter().flat_map(|p| p.bytes().chain([0])).collect();
    group
        .git
        .run_in(
            &[
                "-c",
                "annex.thin=false",
                "annex",
                "add",
                "--batch",
                "-z",
                "--no-check-gitignore",
                "--json",
            ],
            &input,
        )
        .await
        .unwrap();
    // Observe only correction, excluding fixture creation and ingestion.
    std::fs::write(&trace, b"").unwrap();
    Some(Fixture {
        temp,
        group,
        paths,
        trace,
    })
}

#[tokio::test]
async fn correcting_modes_does_not_ingest_a_save_after_annex_add() {
    let Some(f) = fixture().await else { return };
    let before = f.group.git.index().await.unwrap();
    let file = f.temp.path().join(&f.paths[0]);
    std::fs::write(&file, b"a save begun after explicit ingestion\n").unwrap();
    correct(&f.group, &f.paths).await.unwrap();
    let after = f.group.git.index().await.unwrap();
    for (n, path) in f.paths.iter().enumerate() {
        assert_eq!(
            after[path].oid, before[path].oid,
            "mode correction ingested new working bytes"
        );
        assert_eq!(
            after[path].mode,
            if n % 2 == 0 { "100755" } else { "100644" }
        );
    }
    assert_eq!(
        std::fs::read(file).unwrap(),
        b"a save begun after explicit ingestion\n"
    );
    crate::capture_schedule::checked(&f.group, &f.paths, &after).await;
    let stat = lstat(&f.temp.path().join(&f.paths[0])).unwrap().unwrap();
    assert!(
        !f.group
            .lock()
            .capture_schedule
            .verified(&f.paths[0], stat, &after[&f.paths[0]]),
        "new working bytes acquired the old pointer's proof"
    );
}

#[tokio::test]
async fn correcting_modes_never_starts_an_annex_clean_filter() {
    let Some(f) = fixture().await else { return };
    correct(&f.group, &f.paths).await.unwrap();
    let events: Vec<serde_json::Value> = std::fs::read_to_string(f.trace)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert!(
        events.iter().any(|e| e["event"] == "start"),
        "native trace was empty"
    );
    assert_eq!(
        events.iter().filter(|e| e["event"] == "start").count(),
        2,
        "mode correction must use one bounded index read and one index write"
    );
    let filters = events
        .iter()
        .filter(|e| {
            e["event"] == "child_start"
                && e["argv"].as_array().is_some_and(|args| {
                    args.iter().any(|arg| {
                        arg.as_str().is_some_and(|arg| {
                            arg.contains("git-annex")
                                && (arg.contains("filter-process") || arg.contains("smudge"))
                        })
                    })
                })
        })
        .count();
    assert_eq!(
        filters, 0,
        "correcting only modes invoked annex's clean filter"
    );
}

#[test]
fn incomplete_duplicate_unmerged_or_mismatched_entries_fail_closed() {
    let oid = "a".repeat(40);
    let one = format!("100644 {oid} 0\ta\0");
    let expected = [("a", "100755"), ("b", "100644")].into();
    let cases = [
        Vec::new(),
        one.as_bytes().to_vec(),
        format!("{one}{one}").into_bytes(),
        format!("{one}100644 {oid} 0\tc\0").into_bytes(),
        format!("{one}100644 {oid} 1\tb\0").into_bytes(),
        format!("{one}120000 {oid} 0\tb\0").into_bytes(),
        format!("{one}100644 {} 0\tb\0", "0".repeat(40)).into_bytes(),
        format!("{one}100644 not-an-oid 0\tb\0").into_bytes(),
        format!("{one}100644 {oid} 0\tb").into_bytes(),
        format!("{one}\0").into_bytes(),
        vec![0xff, 0],
    ];
    for case in cases {
        assert!(
            index_info(&case, &expected).is_err(),
            "invalid entries supplied an update"
        );
    }
}

#[test]
fn index_info_retains_literal_names_oids_and_unchanged_modes() {
    let oid = "b".repeat(40);
    let modes = [("--option\tline\nname", "100755"), (":(glob)*", "100644")].into();
    let output = format!(
        "100644 {oid} 0\t--option\tline\nname\0\
                          100644 {oid} 0\t:(glob)*\0"
    );
    assert_eq!(
        index_info(output.as_bytes(), &modes).unwrap(),
        format!("100755 {oid}\t--option\tline\nname\0").into_bytes()
    );
}

#[tokio::test]
async fn a_missing_staged_pointer_cannot_partially_update_other_modes() {
    let Some(f) = fixture().await else { return };
    f.group
        .git
        .run(&["update-index", "--force-remove", "--", &f.paths[0]])
        .await
        .unwrap();
    let before = f.group.git.index().await.unwrap();
    assert!(correct(&f.group, &f.paths).await.is_err());
    assert_eq!(f.group.git.index().await.unwrap(), before);
}
