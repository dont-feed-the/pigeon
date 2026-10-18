//! Native pipe faults must fail inventory reads as a whole (03 §3.7, ADR 0048).
//! The subprocess supplies controlled incomplete replies and exits successfully.
#![allow(
    clippy::unwrap_used,
    reason = "isolated native Git reply fault fixtures"
)]

use std::os::unix::fs::PermissionsExt;

use super::*;

fn faulty_git(mode: &str) -> (tempfile::TempDir, Git) {
    let root = tempfile::tempdir().unwrap();
    let bin = root.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let script = bin.join("git");
    let body = r#"#!/usr/bin/env python3
import sys
a='a'*40;b='b'*40
mode=MODE
payload=b'/annex/objects/SHA256E-s1--'+b'c'*64+b'\n'
if 'rev-list' in sys.argv:
    sys.stdin.buffer.read();print(a);print(b)
elif 'ls-tree' in sys.argv:
    sys.stdout.write('100644 blob '+a+'\tone\0'+'100644 blob '+b+'\ttwo\0')
elif any(arg.startswith('--batch-check=') for arg in sys.argv):
    sys.stdin.buffer.read();print(a+' blob '+str(len(payload)))
elif '--batch' in sys.argv:
    names=sys.stdin.buffer.read().splitlines()
    for i,name in enumerate(names):
        if i and mode=='short-reply':break
        if i==0 and mode=='missing-pointer':
            sys.stdout.buffer.write(name+b' missing\n')
        else:
            sys.stdout.buffer.write(name+b' blob '+str(len(payload)).encode()+b'\n'+payload+b'\n')
else:sys.exit(9)
"#;
    std::fs::write(&script, body.replace("MODE", &format!("{mode:?}"))).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o700)).unwrap();
    let git = Git::new(
        root.path(),
        GitEnv {
            path_first: vec![bin],
            extra: Vec::new(),
        },
    );
    (root, git)
}

#[tokio::test]
async fn a_successful_child_with_a_short_object_reply_is_an_inventory_error() {
    let (_root, git) = faulty_git("short-reply");
    let result = git.objects(&["a".repeat(40), "b".repeat(40)]).await;
    assert!(
        matches!(result, Err(Error::Parse { .. })),
        "short reply became {result:?}"
    );
}

#[tokio::test]
async fn a_short_size_reading_cannot_omit_retained_history() {
    let (_root, git) = faulty_git("complete-payloads");
    let result = git.retained_keys(&["HEAD".into()]).await;
    assert!(
        matches!(result, Err(Error::Parse { .. })),
        "short metadata became {result:?}"
    );
}

#[tokio::test]
async fn a_missing_current_pointer_cannot_become_a_smaller_key_inventory() {
    let (_root, git) = faulty_git("missing-pointer");
    let result = git.keys("HEAD").await;
    assert!(
        matches!(result, Err(Error::Parse { .. })),
        "missing pointer became {result:?}"
    );
}

#[tokio::test]
async fn native_git_preserves_binary_objects_and_refuses_a_missing_current_pointer() {
    let root = tempfile::tempdir().unwrap();
    let git = Git::new(root.path(), GitEnv::default());
    git.run(&["init", "-q"]).await.unwrap();
    git.run(&["config", "user.name", "object fixture"])
        .await
        .unwrap();
    git.run(&["config", "user.email", "objects@example.invalid"])
        .await
        .unwrap();
    let content = b"binary\0\n\xffbytes";
    let binary = git
        .run_in(&["hash-object", "-w", "--stdin"], content)
        .await
        .unwrap()
        .trim()
        .to_owned();
    assert_eq!(
        git.objects(&[binary]).await.unwrap(),
        vec![Some(("blob".into(), content.to_vec()))]
    );
    let key = format!("SHA256E-s1--{}", "c".repeat(64));
    let pointer = format!("/annex/objects/{key}\n");
    let oid = git
        .run_in(&["hash-object", "-w", "--stdin"], pointer.as_bytes())
        .await
        .unwrap()
        .trim()
        .to_owned();
    let entry = format!("100644,{oid},file with space");
    git.run(&["update-index", "--add", "--cacheinfo", &entry])
        .await
        .unwrap();
    let tree = git.run(&["write-tree"]).await.unwrap().trim().to_owned();
    let commit = git
        .commit_tree(&tree, &[], "complete inventory")
        .await
        .unwrap();
    assert_eq!(
        git.keys(&commit).await.unwrap(),
        BTreeMap::from([("file with space".into(), key.clone())])
    );
    assert_eq!(
        git.retained_keys(std::slice::from_ref(&commit))
            .await
            .unwrap(),
        BTreeSet::from([key])
    );
    let path = root
        .path()
        .join(".git/objects")
        .join(&oid[..2])
        .join(&oid[2..]);
    std::fs::remove_file(path).unwrap();
    // Native Git itself returns an explicit missing record with exit status 0.
    assert_eq!(git.objects(&[oid]).await.unwrap(), vec![None]);
    assert!(matches!(git.keys(&commit).await, Err(Error::Parse { .. })));
    assert!(git.retained_keys(&[commit]).await.is_err());
}
