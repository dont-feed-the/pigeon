//! Native large captures cross both file-count and byte-input boundaries.
//! Every key, immutable object, working byte and executable mode must survive.

use super::*;
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn large_capture_keeps_every_name_byte_and_executable_mode() {
    let Some(l) = laptop().await else { return };
    let mut expected = BTreeMap::new();
    for i in 0..1700usize {
        let path = if i < 1100 {
            format!("a-{i:04}.bin")
        } else {
            format!("z-{i:04}-{}.bin", "é".repeat(80))
        };
        let bytes = format!("unique captured bytes {i}\n").into_bytes();
        write(&l, &path, &bytes);
        let executable = i % 11 == 0;
        if executable {
            std::fs::set_permissions(l.wt.join(&path), std::fs::Permissions::from_mode(0o755))
                .unwrap();
        }
        expected.insert(path, (bytes, executable));
    }
    let nonportable = "a-held\nline.bin";
    write(&l, nonportable, b"a nonportable name stays local\n");
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap().added,
        expected.len()
    );
    assert!(pigeond::held_file::read(&l.g).names().contains(nonportable));
    assert_eq!(
        std::fs::read(l.wt.join(nonportable)).unwrap(),
        b"a nonportable name stays local\n"
    );
    let tree = l.git.tree("main").await.unwrap();
    let keys = l.git.keys("main").await.unwrap();
    assert_eq!(keys.len(), expected.len());
    let mut objects = BTreeMap::new();
    let mut pending = vec![l.g.git_dir.join("annex/objects")];
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                pending.push(entry.path());
            } else if entry.file_type().unwrap().is_file() {
                objects.insert(
                    entry.file_name().to_string_lossy().into_owned(),
                    entry.path(),
                );
            }
        }
    }
    for (path, (bytes, executable)) in expected {
        assert_eq!(std::fs::read(l.wt.join(&path)).unwrap(), bytes);
        assert_eq!(
            tree[&path].mode,
            if executable { "100755" } else { "100644" }
        );
        let digest = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let key = &keys[&path];
        assert!(key.contains(&format!("--{digest}")));
        assert_eq!(std::fs::read(&objects[key]).unwrap(), bytes);
    }
    assert_eq!(
        capture::capture(&l.g, &dev(1).0, &l.t).await.unwrap().added,
        0
    );
}
