//! A bounded, version-checked request over a peer's private local socket.
//! Each exchange has its own connection; progress never counts as completion.

use std::path::Path;
use std::time::Duration;

use pigeon_proto::{Envelope, LineDecoder, PROTOCOL_VERSION, Request, Response, encode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

async fn next(
    stream: &mut UnixStream,
    decoder: &mut LineDecoder,
) -> Result<Envelope<Response>, String> {
    let mut buffer = vec![0; 64 * 1024];
    loop {
        if let Some(r) = decoder.next_line().map_err(|e| e.to_string())? {
            return Ok(r);
        }
        let n = stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("The Pigeon peer closed its connection".into());
        }
        decoder.push(&buffer[..n]);
    }
}

pub async fn call(socket: &Path, request: Request) -> Result<Response, String> {
    tokio::time::timeout(Duration::from_secs(600), async {
        let mut stream = UnixStream::connect(socket)
            .await
            .map_err(|e| e.to_string())?;
        let mut decoder = LineDecoder::new();
        stream
            .write_all(
                &encode(
                    1,
                    &Request::Hello {
                        version: PROTOCOL_VERSION,
                    },
                )
                .map_err(|e| e.to_string())?,
            )
            .await
            .map_err(|e| e.to_string())?;
        let hello = next(&mut stream, &mut decoder).await?;
        if hello.id != 1
            || hello.body
                != (Response::Hello {
                    version: PROTOCOL_VERSION,
                })
        {
            return Err("Pigeon socket protocol version mismatch".into());
        }
        stream
            .write_all(&encode(2, &request).map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())?;
        loop {
            let answer = next(&mut stream, &mut decoder).await?;
            if answer.id != 2 {
                return Err("Unexpected Pigeon response id".into());
            }
            if !matches!(answer.body, Response::Progress { .. }) {
                return Ok(answer.body);
            }
        }
    })
    .await
    .map_err(|_| "The Pigeon peer did not finish in time".to_owned())?
}

pub async fn bridge(
    socket: &Path,
    operation: pigeon_proto::bridge::Operation,
) -> Result<pigeon_proto::bridge::Reply, String> {
    match call(socket, Request::Bridge { operation }).await? {
        Response::Bridge { result } => Ok(result),
        Response::Problem(p) => Err(format!("Pigeon: {}", p.code)),
        _ => Err("Unexpected bridge response".into()),
    }
}
