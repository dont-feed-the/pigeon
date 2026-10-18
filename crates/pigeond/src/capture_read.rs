//! A capture's independent byte observation (07 §7.2, 15 §15.4–15.5).
//! Bounded, cache-advised reads share Capture's policy. Advancing bytes renew
//! active inactivity time; cancellation cannot publish a late observation.

use std::io::{self, Read};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, Instant};

use pigeon_core::resources::ActiveClock;
use pigeon_git::work::{Class, Control, POLL_MS};
use pigeon_sys::fs::Stat;
use sha2::{Digest, Sha256};

const INACTIVITY: Duration = Duration::from_secs(120);

struct Pending(Option<Arc<dyn Control>>);
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(control) = &self.0 {
            control.ended(Class::Capture);
        }
    }
}
struct Cancel(Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

#[derive(Clone)]
struct State {
    canceled: Arc<AtomicBool>,
    bytes: Arc<AtomicU64>,
    control: Option<Arc<dyn Control>>,
}

impl State {
    fn permitted(&self) -> io::Result<()> {
        loop {
            if self.canceled.load(Ordering::Relaxed) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            if self
                .control
                .as_ref()
                .is_none_or(|c| c.running(Class::Capture))
            {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
    }
    fn charging(&self) -> bool {
        self.control
            .as_ref()
            .is_none_or(|c| c.running(Class::Capture) && c.charging(Class::Capture))
    }
}

async fn supervise<T: Send + 'static>(
    control: Option<Arc<dyn Control>>,
    timeout: Duration,
    work: impl FnOnce(&State) -> io::Result<T> + Send + 'static,
) -> io::Result<T> {
    if let Some(c) = &control {
        c.began(Class::Capture);
    }
    let _pending = Pending(control.clone());
    let state = State {
        control,
        canceled: Arc::new(AtomicBool::new(false)),
        bytes: Arc::new(AtomicU64::new(0)),
    };
    let _cancel = Cancel(state.canceled.clone());
    let worker_state = state.clone();
    let mut worker = tokio::task::spawn_blocking(move || work(&worker_state));
    let epoch = Instant::now();
    let ms = |duration: Duration| duration.as_millis().min(u128::from(u64::MAX)) as u64;
    let mut clock = ActiveClock::new(0, state.charging());
    let mut previous = 0;
    loop {
        let now = ms(epoch.elapsed());
        clock.observe(now, state.charging());
        let bytes = state.bytes.load(Ordering::Relaxed);
        if bytes > previous {
            previous = bytes;
            clock = ActiveClock::new(now, state.charging());
        }
        if clock.used() >= ms(timeout) {
            return Err(io::ErrorKind::TimedOut.into());
        }
        tokio::select! {
            biased;
            result = &mut worker => return result.map_err(io::Error::other)?,
            _ = tokio::time::sleep(Duration::from_millis(POLL_MS).min(Duration::from_millis(clock.remaining(ms(timeout))))) => {},
        }
    }
}

fn digest(reader: &mut impl Read, state: &State) -> io::Result<(u64, String)> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0u8; 65_536];
    let mut size = 0u64;
    loop {
        state.permitted()?;
        let n = reader.read(&mut buffer)?;
        if state.canceled.load(Ordering::Relaxed) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        if n == 0 {
            break;
        }
        size = size
            .checked_add(n as u64)
            .ok_or_else(|| io::Error::other("file size overflow"))?;
        hash.update(&buffer[..n]);
        state.bytes.store(size, Ordering::Relaxed);
    }
    Ok((size, crate::util::hex(&hash.finalize())))
}

pub(crate) async fn checked(
    path: PathBuf,
    expected: Stat,
    control: Option<Arc<dyn Control>>,
) -> io::Result<Option<(u64, String)>> {
    supervise(control, INACTIVITY, move |state| {
        let Some(mut file) =
            pigeon_sys::fs::streaming_read_exact(&path, expected).map_err(io::Error::other)?
        else {
            return Ok(None);
        };
        let observation = digest(&mut file, state)?;
        if file.stat().map_err(io::Error::other)? != expected {
            return Ok(None);
        }
        Ok(Some(observation))
    })
    .await
}

#[cfg(test)]
#[path = "capture_read_tests.rs"]
mod tests;
