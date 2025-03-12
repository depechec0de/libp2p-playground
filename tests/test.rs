use std::time::Duration;

use libp2p::{Multiaddr, futures::SinkExt};
use libp2p_playground::node::{Node, Request};
use tokio::time::sleep;
use tracing_subscriber::EnvFilter;

#[tokio::test]
async fn simple() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init();

    let _n0 = Node::spawn(8000, 0).unwrap();
    let mut n0_addr: Multiaddr = "/ip4/127.0.0.1/tcp/8000".parse().unwrap();
    n0_addr = n0_addr
        .with_p2p(
            "12D3KooWDpJ7As7BWAwRMfu1VU2WCqNjvq387JEYKDBj4kx6nXTN"
                .parse()
                .unwrap(),
        )
        .unwrap();
    let mut n1 = Node::spawn(8001, 1).unwrap();
    let n1_addr = Multiaddr::empty()
        .with_p2p(
            "12D3KooWPjceQrSwdWXPyLLeABRXmuqt69Rg3sBYbU1Nft9HyQ6X"
                .parse()
                .unwrap(),
        )
        .unwrap();
    let mut n2 = Node::spawn(8002, 2).unwrap();
    let n2_addr = Multiaddr::empty()
        .with_p2p(
            "12D3KooWH3uVF6wv47WnArKHk5p6cvgCJEb74UTmxztmQDc298L3"
                .parse()
                .unwrap(),
        )
        .unwrap();

    tokio::try_join!(n1.dial(n0_addr.clone()), n2.dial(n0_addr)).unwrap();

    println!("Peers connected to bootstrap");

    n1.dial(n2_addr.clone()).await.unwrap();

    println!("Peer n1 connected to n2");

    let response = n1
        .send_request(n2_addr.clone(), "Hello, n0!".to_string())
        .await
        .unwrap();

    assert_eq!("Bye!", response);
    println!("First assert passed");

    //n1.dial(n2_addr.clone()).await.unwrap();

    let response = n1
        .send_request(n2_addr, "Hello, n0!".to_string())
        .await
        .unwrap();

    assert_eq!("Bye!", response);
    println!("Second assert passed");
}
