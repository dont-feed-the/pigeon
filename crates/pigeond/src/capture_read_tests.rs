//! Real byte hashing, controlled pauses/stalls and cancellation without a
//! late observation. Production has no test-only I/O or clock switch.
#![allow(clippy::unwrap_used, reason = "capture read lifetime contracts")]
use super::*;
use std::sync::atomic::AtomicUsize;

#[derive(Debug, Default)]
struct Gate {
    run: AtomicBool,
    pending: AtomicUsize,
}
impl Control for Gate {
    fn began(&self, class: Class) {
        assert_eq!(class, Class::Capture);
        self.pending.fetch_add(1, Ordering::Relaxed);
    }
    fn ended(&self, class: Class) {
        assert_eq!(class, Class::Capture);
        self.pending.fetch_sub(1, Ordering::Relaxed);
    }
    fn running(&self, class: Class) -> bool {
        assert_eq!(class, Class::Capture);
        self.run.load(Ordering::Relaxed)
    }
}
struct Slow {
    remaining: usize,
    delay: Duration,
}
impl Read for Slow {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.remaining == 0 || out.is_empty() {
            return Ok(0);
        }
        std::thread::sleep(self.delay);
        self.remaining -= 1;
        out[0] = 42;
        Ok(1)
    }
}
struct Finished(Arc<AtomicBool>);
impl Drop for Finished {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}
async fn until(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while !predicate() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn advancing_capture_bytes_outlive_the_inactivity_budget() {
    let started = Instant::now();
    let got = supervise(None, Duration::from_millis(800), |state| {
        digest(
            &mut Slow {
                remaining: 8,
                delay: Duration::from_millis(200),
            },
            state,
        )
    })
    .await
    .unwrap();
    assert!(started.elapsed() > Duration::from_millis(800));
    assert_eq!(got, (8, crate::util::hex(&Sha256::digest([42u8; 8]))));
}

#[tokio::test]
async fn a_deliberate_pause_spends_no_read_budget_and_resumes_exact_bytes() {
    let gate = Arc::new(Gate::default());
    let task = tokio::spawn(supervise(
        Some(gate.clone()),
        Duration::from_millis(300),
        |state| digest(&mut io::Cursor::new(b"exact bytes"), state),
    ));
    until(|| gate.pending.load(Ordering::Relaxed) == 1).await;
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert!(!task.is_finished());
    gate.run.store(true, Ordering::Relaxed);
    assert_eq!(
        task.await.unwrap().unwrap(),
        (11, crate::util::hex(&Sha256::digest(b"exact bytes")))
    );
    assert_eq!(gate.pending.load(Ordering::Relaxed), 0);
}

#[tokio::test]
async fn canceling_a_paused_read_releases_capture_and_ends_the_worker() {
    let gate = Arc::new(Gate::default());
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = finished.clone();
    let task = tokio::spawn(supervise(
        Some(gate.clone()),
        Duration::from_millis(300),
        move |state| {
            let _done = Finished(worker_finished);
            digest(&mut io::Cursor::new(b"never read while held"), state)
        },
    ));
    until(|| gate.pending.load(Ordering::Relaxed) == 1).await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(gate.pending.load(Ordering::Relaxed), 0);
    until(|| finished.load(Ordering::Relaxed)).await;
}

#[tokio::test]
async fn a_stalled_read_times_out_without_publishing_its_late_result() {
    let gate = Arc::new(Gate::default());
    gate.run.store(true, Ordering::Relaxed);
    let published = Arc::new(AtomicBool::new(false));
    let publish = published.clone();
    let finished = Arc::new(AtomicBool::new(false));
    let worker_finished = finished.clone();
    let error = supervise(
        Some(gate.clone()),
        Duration::from_millis(200),
        move |state| {
            let _done = Finished(worker_finished);
            let result = digest(
                &mut Slow {
                    remaining: 1,
                    delay: Duration::from_millis(800),
                },
                state,
            )?;
            publish.store(true, Ordering::Relaxed);
            Ok(result)
        },
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert_eq!(gate.pending.load(Ordering::Relaxed), 0);
    until(|| finished.load(Ordering::Relaxed)).await;
    assert!(!published.load(Ordering::Relaxed));
}

#[tokio::test]
async fn the_opened_inode_must_match_the_expected_native_observation() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("saved");
    std::fs::write(&path, b"observed bytes").unwrap();
    let stat = pigeon_sys::fs::lstat(&path).unwrap().unwrap();
    assert_eq!(
        checked(path.clone(), stat, None).await.unwrap(),
        Some((14, crate::util::hex(&Sha256::digest(b"observed bytes"))))
    );
    let mut wrong = stat;
    wrong.id.ino = stat.id.ino.wrapping_add(1);
    assert_eq!(checked(path, wrong, None).await.unwrap(), None);
}
