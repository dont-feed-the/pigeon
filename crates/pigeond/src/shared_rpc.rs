//! The storage node's private, version-checked local socket. File bytes stay
//! outside JSON; each exchange is bounded and progress is never completion.

use std::path::Path;
use std::time::Duration;

use pigeon_proto::bridge::{Operation, Reply};
use pigeon_proto::{Envelope, LineDecoder, PROTOCOL_VERSION, Request, Response, encode};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

async fn next(
    stream: &mut UnixStream,
    decoder: &mut LineDecoder,
) -> Result<Envelope<Response>, String> {
    let mut buffer = vec![0; 64 * 1024];
    loop {
        if let Some(line) = decoder.next_line().map_err(|e| e.to_string())? {
            return Ok(line);
        }
        let count = stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
        if count == 0 {
            return Err("The content node closed its connection".into());
        }
        decoder.push(&buffer[..count]);
    }
}

pub(crate) async fn call(socket: &Path, operation: Operation) -> Result<Reply, String> {
    tokio::time::timeout(Duration::from_secs(60), async {
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
            return Err("Content node socket protocol version mismatch".into());
        }
        stream
            .write_all(&encode(2, &Request::Bridge { operation }).map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())?;
        loop {
            let answer = next(&mut stream, &mut decoder).await?;
            if answer.id != 2 {
                return Err("Unexpected content node response".into());
            }
            match answer.body {
                Response::Progress { .. } => {}
                Response::Bridge { result } => return Ok(result),
                _ => return Err("The content node refused the request".into()),
            }
        }
    })
    .await
    .map_err(|_| "The content node did not finish in time".to_owned())?
}
