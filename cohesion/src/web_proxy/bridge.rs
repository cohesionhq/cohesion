//! Relays one browser WebSocket to its own cohesiond session.

use axum::extract::ws::{Message, WebSocket};
use futures_util::SinkExt;

use crate::connection::Upstream;

/// Passes text frames both ways unchanged until either side closes, then closes the other.
pub async fn relay(mut browser: WebSocket, mut upstream: Upstream) {
    loop {
        tokio::select! {
            message = browser.recv() => match message {
                Some(Ok(Message::Text(text))) => {
                    if upstream.send(text.to_string()).await.is_err() {
                        break;
                    }
                }
                Some(Ok(Message::Close(_))) => {
                    // Sends the close reply already queued, finishing the handshake.
                    let _ = browser.flush().await;
                    return;
                }
                Some(Err(_)) | None => return,
                Some(Ok(_)) => {}
            },
            frame = upstream.recv() => match frame {
                Some(frame) => {
                    if browser.send(Message::text(frame)).await.is_err() {
                        return;
                    }
                }
                None => break,
            },
        }
    }
    // Dropping `upstream` ends the daemon's session.
    let _ = browser.send(Message::Close(None)).await;
}
