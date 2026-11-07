use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt, duplex};

use super::FirstByte;

const WITHIN: Duration = Duration::from_secs(10);

#[tokio::test(start_paused = true)]
async fn a_silent_peer_times_out() {
    let (_client, server) = duplex(64);
    let mut server = FirstByte::new(server, WITHIN);
    let mut byte = [0u8; 1];
    let err = server.read(&mut byte).await.unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test(start_paused = true)]
async fn the_first_byte_disarms_the_deadline() {
    let (mut client, server) = duplex(64);
    let mut server = FirstByte::new(server, WITHIN);
    client.write_all(b"x").await.unwrap();
    let mut byte = [0u8; 1];
    server.read_exact(&mut byte).await.unwrap();
    tokio::time::advance(WITHIN * 100).await;
    client.write_all(b"y").await.unwrap();
    server.read_exact(&mut byte).await.unwrap();
    assert_eq!(&byte, b"y");
}
