//! Joining/recovery materialization must not invent empty bytes when a
//! committed object is missing, including refounded joins in place
//! (09 §9.9, ADR 0048).
#![allow(clippy::unwrap_used, reason = "isolated real Git object loss fixtures")]

use super::*;
use pigeon_git::GitEnv;
use std::collections::BTreeMap;

struct Fixture {
    root: tempfile::TempDir,
    group: Group,
    main: String,
    objects: BTreeMap<String, String>,
}

async fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let git = crate::git::Git::new(root.path(), GitEnv::default());
    git.run(&["init", "-q", "-b", "main"]).await.unwrap();
    git.run(&["config", "user.name", "Recovery fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "recovery@example.invalid"])
        .await
        .unwrap();
    let pointer = format!("/annex/objects/SHA256E-s1--{}\n", "a".repeat(64));
    let mut objects = BTreeMap::new();
    for (name, bytes) in [
        ("binary", b"binary\0\xff\nbytes".as_slice()),
        ("empty", b"".as_slice()),
        ("hidden", pointer.as_bytes()),
    ] {
        let oid = git
            .run_in(&["hash-object", "-w", "--stdin"], bytes)
            .await
            .unwrap()
            .trim()
            .to_owned();
        git.run(&[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("100644,{oid},{name}"),
        ])
        .await
        .unwrap();
        objects.insert(name.to_owned(), oid);
    }
    let tree = git.run(&["write-tree"]).await.unwrap().trim().to_owned();
    let main = git
        .commit_tree(&tree, &[], "recovery fixture")
        .await
        .unwrap();
    git.update_ref("refs/heads/main", &main, None)
        .await
        .unwrap();
    let group = Group::open(&"b".repeat(40), root.path(), &GitEnv::default())
        .await
        .unwrap();
    Fixture {
        root,
        group,
        main,
        objects,
    }
}

#[tokio::test]
async fn missing_join_objects_refuse_without_replacing_working_bytes() {
    for in_place in [false, true] {
        for name in ["binary", "hidden"] {
            let f = fixture().await;
            let saved = b"work saved while the joining peer was awaited\n";
            std::fs::write(f.root.path().join(name), saved).unwrap();
            let oid = &f.objects[name];
            let loose = f
                .group
                .git_dir
                .join("objects")
                .join(&oid[..2])
                .join(&oid[2..]);
            std::fs::remove_file(loose).unwrap();
            assert!(f.group.git.object(oid).await.is_err());
            let result = materialize(&f.group, &f.main, in_place).await;
            assert!(result.is_err(), "missing {name} object became success");
            assert_eq!(std::fs::read(f.root.path().join(name)).unwrap(), saved);
        }
    }
}

#[tokio::test]
async fn complete_join_objects_keep_binary_empty_and_hidden_states_distinct() {
    for in_place in [false, true] {
        let f = fixture().await;
        materialize(&f.group, &f.main, in_place).await.unwrap();
        assert_eq!(
            std::fs::read(f.root.path().join("binary")).unwrap(),
            b"binary\0\xff\nbytes"
        );
        assert!(
            std::fs::read(f.root.path().join("empty"))
                .unwrap()
                .is_empty()
        );
        assert!(!f.root.path().join("hidden").exists());
        let flags = f.group.git.run(&["ls-files", "-v"]).await.unwrap();
        assert!(flags.lines().any(|line| line == "S hidden"));
        let sparse = std::fs::read_to_string(f.group.git_dir.join("info/sparse-checkout")).unwrap();
        assert!(sparse.contains("!/hidden\n"));
    }
}

#[tokio::test]
async fn a_non_blob_object_cannot_become_a_joined_file() {
    for in_place in [false, true] {
        let f = fixture().await;
        let mut raw_tree = b"100644 wrong-kind\0".to_vec();
        raw_tree.extend(crate::util::unhex(&f.main).unwrap());
        let tree = f
            .group
            .git
            .run_in(&["hash-object", "-w", "-t", "tree", "--stdin"], &raw_tree)
            .await
            .unwrap()
            .trim()
            .to_owned();
        let commit = f
            .group
            .git
            .commit_tree(&tree, &[], "wrong object kind fixture")
            .await
            .unwrap();
        // The real Git index/tree readers accept the entry's declared mode;
        // materialization must independently check the object's actual kind.
        f.group.git.run(&["read-tree", &commit]).await.unwrap();
        let files = f.group.git.tree_files(&commit).await.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].oid, f.main);
        let answer = f
            .group
            .git
            .objects(std::slice::from_ref(&f.main))
            .await
            .unwrap();
        assert!(matches!(answer.as_slice(), [Some((kind, _))] if kind == "commit"));
        let saved = b"working notes to keep\n";
        std::fs::write(f.root.path().join("wrong-kind"), saved).unwrap();
        assert_eq!(
            materialize(&f.group, &commit, in_place).await,
            Err("join-files-unreadable".into())
        );
        assert_eq!(
            std::fs::read(f.root.path().join("wrong-kind")).unwrap(),
            saved
        );
    }
}

async fn materialize(g: &Group, main: &str, in_place: bool) -> Result<(), String> {
    if in_place {
        crate::join_in_place::take_tree(g, main).await
    } else {
        hide_all(g, main).await
    }
}
