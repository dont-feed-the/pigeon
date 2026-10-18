//! Recover an actual lost native listener even while a round waits on a
//! slow peer (09 §9.3). The owned supervisor stops before daemon shutdown;
//! group identity and leaving checks prevent revival after removal.

use std::sync::Arc;
use std::time::Duration;

use crate::daemon::Daemon;

pub(crate) async fn run(d: Arc<Daemon>) {
    loop {
        let Some(net) = d.net.get() else { return };
        let groups = d.groups();
        let live = groups.iter().map(|g| g.id.clone()).collect();
        net.listeners.forget_unmanaged_losses(&live);
        for g in groups {
            if !net.listeners.lost(&g.id) {
                continue;
            }
            // One busy group must not delay recovery in other groups, and
            // the loss remains pending until its repository can be checked.
            let Ok(_busy) = g.busy.try_lock() else {
                continue;
            };
            if !d.current_group(&g).await
                || crate::leave::leaving(&g).is_some()
                || !net.listeners.take_lost(&g.id)
            {
                continue;
            }
            tracing::warn!(group = %g.id, "remotedaemon-listener-lost");
            g.stop_remotedaemon().await;
            g.ensure_remotedaemon(&d.wake).await;
            d.wake.notify_one();
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
