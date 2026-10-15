//! Socket-level paging tests: discard interrupted generations, retain complete
//! metadata, bound retries, and reject cursors that could loop or skip paths.
#![allow(clippy::unwrap_used, clippy::panic, reason = "tests")]

use pigeon_proto::bridge::Snapshot;
use pigeon_proto::{Envelope, PROTOCOL_VERSION, Request, Response, encode};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

use super::*;

type Exchange = (Option<Cursor>, Reply);

async fn serve(
    exchanges: Vec<Exchange>,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    tokio::task::JoinHandle<()>,
) {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("peer.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let task = tokio::spawn(async move {
        for (cursor, result) in exchanges {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            let mut line = String::new();
            stream.read_line(&mut line).await.unwrap();
            let hello: Envelope<Request> = serde_json::from_str(&line).unwrap();
            assert_eq!(
                hello.body,
                Request::Hello {
                    version: PROTOCOL_VERSION
                }
            );
            stream
                .get_mut()
                .write_all(
                    &encode(
                        1,
                        &Response::Hello {
                            version: PROTOCOL_VERSION,
                        },
                    )
                    .unwrap(),
                )
                .await
                .unwrap();
            line.clear();
            stream.read_line(&mut line).await.unwrap();
            let request: Envelope<Request> = serde_json::from_str(&line).unwrap();
            assert_eq!(
                request.body,
                Request::Bridge {
                    operation: Operation::Snapshot {
                        cursor,
                        limit: MAX_PAGE_FILES,
                    }
                }
            );
            let wire = encode(2, &Response::Bridge { result }).unwrap();
            assert!(wire.len() <= pigeon_proto::bridge::MAX_PAGE_BYTES);
            stream.get_mut().write_all(&wire).await.unwrap();
        }
    });
    (temp, path, task)
}

fn file(path: String) -> File {
    File {
        path,
        etag: "a".repeat(64),
        size: 5,
        diverging: false,
    }
}

fn page(revision: &str, files: Vec<File>, more: bool) -> Snapshot {
    Snapshot {
        revision: revision.into(),
        next: more.then(|| Cursor {
            revision: revision.into(),
            after: files.last().map(|f| f.path.clone()),
        }),
        files,
    }
}

fn changed() -> Reply {
    Reply::Failure {
        status: 412,
        message: "The catalogue changed".into(),
    }
}

#[tokio::test]
async fn a_large_listing_discards_old_pages_and_only_persists_the_complete_revision() {
    let old = page("old", vec![file("old-only.txt".into())], true);
    let mut exchanges = vec![(None, Reply::Snapshot(old.clone())), (old.next, changed())];
    let files: Vec<_> = (0..4000)
        .map(|i| file(format!("Files/{i:06}-{}.txt", "x".repeat(190))))
        .collect();
    assert!(serde_json::to_vec(&files).unwrap().len() > pigeon_proto::MAX_LINE);
    let mut cursor = None;
    for (i, chunk) in files.chunks(400).enumerate() {
        let page = page("new", chunk.to_vec(), i < 9);
        exchanges.push((cursor, Reply::Snapshot(page.clone())));
        cursor = page.next;
    }
    let (_temp, socket, task) = serve(exchanges).await;
    let catalogue = read(&socket, None).await.unwrap();
    task.await.unwrap();
    assert_eq!(catalogue.revision, "new");
    assert_eq!(catalogue.files.len(), files.len());
    assert!(!catalogue.files.contains_key("old-only.txt"));
    let (_state, app) = crate::tests::app().await;
    let client = crate::tests::client("a", "alice", "app-a");
    let index = index::build(&app, &client, catalogue).await.unwrap();
    assert_eq!(index.nodes.len(), files.len() + 2);
    assert_eq!(index.nodes[""].size, 20000);
    let store = app.store.lock().await;
    assert_eq!(store.clients[&client.id].metadata.len(), files.len() + 2);
    assert!(
        !store.clients[&client.id]
            .metadata
            .contains_key("old-only.txt")
    );
}

#[tokio::test]
async fn continually_changing_listings_stop_and_post_save_reads_never_switch_revisions() {
    let (_temp, socket, task) = serve(vec![
        (None, changed()),
        (None, changed()),
        (None, changed()),
    ])
    .await;
    let error = read(&socket, None).await.err().unwrap();
    assert_eq!(error.0, StatusCode::SERVICE_UNAVAILABLE);
    task.await.unwrap();
    let required = Cursor {
        revision: "saved".into(),
        after: None,
    };
    let (_temp, socket, task) = serve(vec![(Some(required), changed())]).await;
    let error = read(&socket, Some("saved")).await.err().unwrap();
    assert_eq!(error.0, StatusCode::PRECONDITION_FAILED);
    task.await.unwrap();
}

#[tokio::test]
async fn invalid_continuations_and_mixed_or_unordered_pages_are_rejected() {
    let first = page("one", vec![file("a.txt".into())], true);
    let invalid = [
        page("two", vec![file("b.txt".into())], false),
        page("one", vec![file("a.txt".into())], false),
        page(
            "one",
            vec![file("z.txt".into()), file("b.txt".into())],
            false,
        ),
        page("one", vec![file("../escape".into())], false),
        page("one", vec![], true),
        Snapshot {
            next: Some(Cursor {
                revision: "one".into(),
                after: Some("z.txt".into()),
            }),
            ..page("one", vec![file("b.txt".into())], true)
        },
    ];
    for invalid in invalid {
        let (_temp, socket, task) = serve(vec![
            (None, Reply::Snapshot(first.clone())),
            (first.next.clone(), Reply::Snapshot(invalid)),
        ])
        .await;
        let error = read(&socket, None).await.err().unwrap();
        assert_eq!(error.0, StatusCode::SERVICE_UNAVAILABLE);
        task.await.unwrap();
    }
}
