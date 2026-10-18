//! Metadata reads retain complete key binding while reducing native startup work.
#![allow(clippy::unwrap_used, reason = "isolated metadata-query pipe fixtures")]

use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn complete_metadata_sweeps_do_not_start_a_child_per_sixty_four_keys() {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let count = root.path().join("starts");
    let body = r#"#!/usr/bin/env python3
import sys,json
with open(COUNTER,'a') as f:f.write('start\n')
for key in sys.stdin.read().splitlines():
    if 'whereis' in sys.argv:
        print(json.dumps(dict(key=key,whereis=[dict(uuid='known-backup')])))
    elif 'contentlocation' in sys.argv:print('')
    else:sys.exit(9)
"#;
    let script = bin.join("git");
    std::fs::write(
        &script,
        body.replace("COUNTER", &format!("{:?}", count.to_str().unwrap())),
    )
    .unwrap();
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let git = crate::git::Git::new(
        root.path(),
        pigeon_git::GitEnv {
            path_first: vec![bin],
            extra: Vec::new(),
        },
    );
    let keys: BTreeSet<_> = (0..1025).map(|n| format!("SHA256E-s1--{n:064x}")).collect();
    let holders = super::holders_for(&git, &keys).await;
    assert_eq!(holders.len(), keys.len());
    assert!(
        holders
            .values()
            .all(|locations| locations.contains("known-backup"))
    );
    assert!(super::present_for(&git, &keys).await.is_empty());
    let starts = std::fs::read_to_string(count).unwrap().lines().count();
    assert_eq!(
        starts, 11,
        "complete 1025-key sweeps started too many native children"
    );
}

#[tokio::test]
async fn native_metadata_batches_keep_every_present_object_and_absent_answer_bound() {
    let root = tempfile::tempdir().unwrap();
    let git = crate::git::Git::new(root.path(), pigeon_git::GitEnv::default());
    let Ok(version) = git.run(&["annex", "version", "--raw"]).await else {
        assert!(std::env::var_os("PIGEON_REQUIRE_GIT_ANNEX").is_none());
        return;
    };
    git.run(&["init", "-q"]).await.unwrap();
    git.run(&["config", "user.name", "metadata fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "metadata@example.invalid"])
        .await
        .unwrap();
    git.run(&["annex", "init", "metadata"]).await.unwrap();
    let paths: Vec<_> = (0..300).map(|n| format!("file-{n:03}.bin")).collect();
    let mut input = Vec::new();
    let mut data = Vec::new();
    for (n, path) in paths.iter().enumerate() {
        let bytes = format!("independent native bytes {n}\n").into_bytes();
        std::fs::write(root.path().join(path), &bytes).unwrap();
        data.push(bytes);
        input.extend_from_slice(path.as_bytes());
        input.push(0);
    }
    let answer = git
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
    let answers: Vec<pigeon_git::annex::Answer> = answer
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(answers.len(), paths.len());
    let mut expected = std::collections::BTreeMap::new();
    for ((answer, path), bytes) in answers.into_iter().zip(paths).zip(data) {
        assert!(answer.success);
        assert_eq!(answer.file.as_deref(), Some(path.as_str()));
        expected.insert(answer.key.unwrap(), bytes);
    }
    let absent = format!("SHA256E-s1--{}", "f".repeat(64));
    assert!(!expected.contains_key(&absent));
    let mut keys: BTreeSet<_> = expected.keys().cloned().collect();
    keys.insert(absent.clone());
    let holders = super::holders_for(&git, &keys).await;
    assert_eq!(holders.len(), keys.len());
    let pinned = include_str!("../../../third_party/git-annex/VERSION")
        .lines()
        .find_map(|line| line.strip_prefix("PIGEON_VERSION="))
        .unwrap()
        .trim_matches('"');
    if version.trim() == pinned {
        assert_eq!(
            git.whereis_no_urls.get(),
            Some(&true),
            "bundled native optimization was not discovered"
        );
    }
    assert!(holders[&absent].is_empty());
    let uuid = git
        .run(&["config", "annex.uuid"])
        .await
        .unwrap()
        .trim()
        .to_owned();
    assert!(expected.keys().all(|key| holders[key].contains(&uuid)));
    let present = super::present_for(&git, &keys).await;
    assert_eq!(present.len(), expected.len());
    assert!(!present.contains_key(&absent));
    let syncer = crate::net::sync::AnnexSyncer::new(Default::default(), git.repo.env.clone());
    let repository = crate::net::Repository {
        group: "native-metadata".into(),
        git_dir: root.path().join(".git"),
        worktree: Some(root.path().into()),
        role: pigeon_core::filter::Role::Laptop,
        auth_token: None,
        checking: false,
    };
    use crate::net::sync::Syncer;
    for (key, bytes) in &expected {
        assert_eq!(
            std::fs::read(&present[key]).unwrap().as_slice(),
            bytes.as_slice()
        );
        assert!(syncer.sync(&repository, key, true).await.unwrap());
    }
    let (key, _) = expected.first_key_value().unwrap();
    // The private fixture deliberately damages annex's read-only object.
    // Ordinary clients never make an object writable.
    std::fs::set_permissions(&present[key], std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(&present[key], b"later corruption").unwrap();
    assert!(!syncer.sync(&repository, key, true).await.unwrap());
    std::fs::set_permissions(
        present[key].parent().unwrap(),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    std::fs::remove_file(&present[key]).unwrap();
    assert!(syncer.sync(&repository, key, false).await.is_err());
    assert!(syncer.sync(&repository, &absent, false).await.is_err());
}

fn sizes(tc: &hegel::TestCase) -> Vec<String> {
    use hegel::generators as gs;
    tc.draw(gs::vecs(gs::integers::<u16>().max_value(2048)).max_size(600))
        .into_iter()
        .map(|size| "é".repeat(usize::from(size)))
        .collect()
}

#[hegel::test(test_cases = 1000)]
fn metadata_batches_preserve_every_identity_in_order(tc: hegel::TestCase) {
    let keys = sizes(&tc);
    let refs: Vec<_> = keys.iter().collect();
    let batches: Vec<_> = super::metadata_batches(&refs).collect();
    assert!(batches.iter().all(|batch| !batch.is_empty()));
    assert_eq!(
        batches.into_iter().flatten().collect::<Vec<_>>(),
        refs.iter().collect::<Vec<_>>()
    );
}

#[hegel::test(test_cases = 1000)]
fn metadata_batches_bound_key_count_and_encoded_input_bytes(tc: hegel::TestCase) {
    let keys = sizes(&tc);
    let refs: Vec<_> = keys.iter().collect();
    for batch in super::metadata_batches(&refs) {
        assert!(batch.len() <= super::METADATA_KEYS);
        let bytes: usize = batch.iter().map(|key| key.len() + 1).sum();
        assert!(bytes <= super::METADATA_INPUT_BYTES || batch.len() == 1);
    }
}

#[hegel::test(test_cases = 1000)]
fn short_metadata_batches_are_full_before_the_next_key(tc: hegel::TestCase) {
    let keys = sizes(&tc);
    let refs: Vec<_> = keys.iter().collect();
    let batches: Vec<_> = super::metadata_batches(&refs).collect();
    for pair in batches.windows(2) {
        if pair[0].len() < super::METADATA_KEYS {
            let bytes: usize = pair[0].iter().map(|key| key.len() + 1).sum();
            assert!(bytes + pair[1][0].len() + 1 > super::METADATA_INPUT_BYTES);
        }
    }
}

#[hegel::test(test_cases = 1000)]
fn an_oversized_metadata_identity_cannot_starve_the_next_one(tc: hegel::TestCase) {
    use hegel::generators as gs;
    let extra = tc.draw(gs::integers::<u16>().max_value(1024));
    let keys = [
        "x".repeat(super::METADATA_INPUT_BYTES + usize::from(extra)),
        "next".to_owned(),
    ];
    let refs: Vec<_> = keys.iter().collect();
    let batches: Vec<_> = super::metadata_batches(&refs).collect();
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0], &refs[..1]);
    assert_eq!(batches[1], &refs[1..]);
}
