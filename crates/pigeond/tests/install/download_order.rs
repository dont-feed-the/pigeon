//! Native signed-history download plans respect local folder priority,
//! real version dates, shared keys and the existing freed-folder choices.

use super::*;
use pigeon_core::queue::{Priority as QueuePriority, Work};
use pigeond::download_queue::{Priority, items};

#[tokio::test]
async fn kept_downloads_follow_folder_priority_then_date_and_deduplicate_shared_keys() {
    let Some(l) = laptop_fixture(true).await else {
        return;
    };
    let genesis = l.git.rev("refs/heads/main").await.unwrap();
    let mut tree = l.git.tree(&genesis).await.unwrap();
    let old = version(&l, "old.txt", b"old high folder", true).await;
    let newer = version(&l, "new.txt", b"new high folder", true).await;
    let normal = version(&l, "normal.txt", b"normal folder", true).await;
    let low = version(&l, "low.txt", b"low folder", true).await;
    let freed_key = version(&l, "freed.txt", b"freed folder", true).await;
    tree.insert("Freed/old.txt".into(), freed_key);
    tree.insert("Photos/old.txt".into(), old);
    let first = bens(&l, &tree, &genesis, 60).await;
    tree.insert("Photos/new.txt".into(), newer.clone());
    tree.insert("Archives/shared.txt".into(), newer);
    tree.insert("root.txt".into(), normal);
    tree.insert("Archives/low.txt".into(), low);

    let second = bens(&l, &tree, &first, 120).await;
    let keys = l.git.keys(&second).await.unwrap();
    let priorities = [
        ("Photos".into(), Priority::High),
        ("Archives".into(), Priority::Low),
    ]
    .into();
    std::fs::write(l.g.root().join("freed.json"), br#"["Freed"]"#).unwrap();
    let plan = items(&l.g, &priorities).await;
    let ordered = plan
        .iter()
        .map(|work| match work {
            Work::Download { key, .. } => key.clone(),
            _ => panic!("not a download"),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        [
            keys["Photos/new.txt"].clone(),
            keys["Photos/old.txt"].clone(),
            keys["root.txt"].clone(),
            keys["Archives/low.txt"].clone()
        ]
    );
    assert!(matches!(
        plan[0],
        Work::Download {
            priority: QueuePriority::High,
            ..
        }
    ));
    std::fs::write(l.g.root().join("freed.json"), br#"["Photos","Freed"]"#).unwrap();
    let freed = items(&l.g, &priorities).await;
    assert_eq!(freed.len(), 3);
    assert!(freed.iter().all(|work| !matches!(
        work,
        Work::Download {
            priority: QueuePriority::High,
            ..
        }
    )));
    assert!(
        !freed.iter().any(
            |work| matches!(work, Work::Download { key, .. } if key == &keys["Photos/old.txt"])
        )
    );
}
