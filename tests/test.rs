use std::time::Duration;

use libp2p::{Multiaddr, futures::SinkExt};
use libp2p_playground::node::{Command, Node, Request};
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn simple() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let mut n0 = Node::spawn(8000, 0).unwrap();
    let n0_peer_id = Multiaddr::empty()
        .with_p2p(
            "12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN"
                .parse()
                .unwrap(),
        )
        .unwrap();
    let mut n1 = Node::spawn(8001, 1).unwrap();
    let n1_peer_id = Multiaddr::empty()
        .with_p2p(
            "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X"
                .parse()
                .unwrap(),
        )
        .unwrap();
    let mut n2 = Node::spawn(8002, 2).unwrap();
    let n2_peer_id = Multiaddr::empty()
        .with_p2p(
            "12D3KooWH3uVF6wv47WnArKHk5p6cvgCJEb74UTmxztmQDc298L3"
                .parse()
                .unwrap(),
        )
        .unwrap();

    let n0_address: Multiaddr = format!("/ip4/127.0.0.1/tcp/8000").parse().unwrap();
    n1.send(Command::Dial(n0_address.clone())).await.unwrap();
    n2.send(Command::Dial(n0_address)).await.unwrap();

    sleep(Duration::from_millis(50)).await;

    n1.send(Command::SendRequest(Request {
        dest: n2_peer_id,
        payload: "Hello, n0!".to_string(),
    }))
    .await
    .unwrap();

    sleep(Duration::from_millis(50)).await;
}
