//! Optional group telemetry over authenticated iroh (ADR 0025, 12 §12.2).
//! A collector admits only roster members of a group that selected it.
//! Header, payload and response have deadlines; a batch is at most 256 KiB.

use super::socket::Net;
use iroh::endpoint::{RecvStream, SendStream};
use pigeon_proto::stream::{HEADER_VERSION, Refusal, StreamAnswer, StreamHeader};
use std::time::Duration;

pub(super) const MAX_BATCH: usize = 256 * 1024;

pub(super) async fn serve(net: &Net, device: &str, mut send: SendStream, mut recv: RecvStream) {
    let exchange = async {
        let line = super::read_line(&mut recv).await.ok()?;
        let header: StreamHeader = serde_json::from_slice(&line).ok()?;
        let refusal = if header.v != HEADER_VERSION {
            Some(Refusal::PeerTooOld)
        } else if net.directory.telemetry_device(&header.group).as_deref()
            != Some(net.node.device_id().as_str())
        {
            Some(Refusal::UnknownGroup)
        } else if net.directory.admit(&header.group, device).is_none() {
            Some(Refusal::NotInRoster)
        } else {
            None
        };
        if let Some(refusal) = refusal {
            let _ = send
                .write_all(&StreamAnswer::refuse(refusal).to_line())
                .await;
            let _ = send.finish();
            return Some(());
        }
        send.write_all(&StreamAnswer::accept().to_line())
            .await
            .ok()?;
        let payload = recv.read_to_end(MAX_BATCH).await.ok()?;
        let records: Vec<serde_json::Value> = serde_json::from_slice(&payload).ok()?;
        let accepted = match &net.loops {
            Some(loops) => {
                loops
                    .telemetry(header.group, device.to_owned(), records)
                    .await
            }
            None => false,
        };
        send.write_all(if accepted { b"ok\n" } else { b"refused\n" })
            .await
            .ok()?;
        send.finish().ok()?;
        // Give QUIC time to deliver the final acknowledgement.
        let _ = send.stopped().await;
        Some(())
    };
    let _ = tokio::time::timeout(Duration::from_secs(15), exchange).await;
}
