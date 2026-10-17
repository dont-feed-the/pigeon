//! Repeated in-place saves retain every unprotected captured version.
//! Status may invoke Git's annex clean filter before explicit annex add;
//! those observations must never turn the saved object into a mutable link.

use super::*;

#[tokio::test]
async fn repeated_in_place_saves_preserve_every_unprotected_capture() {
    let Some(l) = laptop().await else { return };
    let mut versions = Vec::new();
    for i in 0..4 {
        let bytes = format!("the unprotected version number {i}\n");
        write(&l, "saved.txt", bytes.as_bytes());
        let done = capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap();
        let key = l.git.keys(&done.commit.unwrap()).await.unwrap()["saved.txt"].clone();
        versions.push((key, bytes));
        l.git
            .run(&["update-index", "-q", "--refresh"])
            .await
            .unwrap();
    }
    write(&l, "saved.txt", b"a later save not captured yet\n");
    assert!(capture::dirty(&l.g).await.unwrap().contains("saved.txt"));
    for (key, bytes) in versions {
        let object = pigeond::custody::present(&l.g, &key).await;
        assert!(
            object.is_some(),
            "an unprotected captured object disappeared: {key}"
        );
        assert_eq!(
            std::fs::read(object.unwrap()).unwrap(),
            bytes.as_bytes(),
            "a later save changed {key}"
        );
    }
}
