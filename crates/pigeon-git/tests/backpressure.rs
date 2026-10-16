//! Large Git batches must feed stdin and drain stdout concurrently. Otherwise
//! a catalogue can deadlock once both operating-system pipe buffers are full.
#![allow(clippy::unwrap_used, reason = "tests")]

use std::time::Duration;

use pigeon_git::{Repo, env::GitEnv};

#[tokio::test]
async fn large_cat_file_batches_make_progress_in_both_directions() {
    let temp = tempfile::tempdir().unwrap();
    let repo = Repo::new(temp.path(), GitEnv::default());
    let timeout = Duration::from_secs(5);
    repo.run(&["init", "-q"], None, timeout).await.unwrap();
    let blob = vec![b'x'; 256];
    let oid = repo
        .run(&["hash-object", "-w", "--stdin"], Some(&blob), timeout)
        .await
        .unwrap()
        .text()
        .trim()
        .to_owned();
    let count = 16384;
    let input = format!("{oid}\n").repeat(count);
    let out = repo
        .run(&["cat-file", "--batch"], Some(input.as_bytes()), timeout)
        .await
        .unwrap();
    let mut record = format!("{oid} blob {}\n", blob.len()).into_bytes();
    record.extend_from_slice(&blob);
    record.push(b'\n');
    assert_eq!(out.stdout.len(), record.len() * count);
    assert!(
        out.stdout
            .chunks_exact(record.len())
            .all(|chunk| chunk == record)
    );
}
