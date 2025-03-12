use std::error::Error;

use libp2p::{
    Multiaddr,
    futures::{
        SinkExt,
        channel::{mpsc, oneshot},
    },
};
use node::Request;

pub mod node;

pub(crate) type ClientResponder<T> = oneshot::Sender<Result<T, Box<dyn Error + Send>>>;

pub enum Command {
    Dial {
        address: Multiaddr,
        resp: ClientResponder<()>,
    },
    SendRequest {
        dest: Multiaddr,
        payload: String,
        resp: ClientResponder<String>,
    },
}

#[derive(Clone)]
pub struct NodeClient {
    sender: mpsc::Sender<Command>,
}

impl NodeClient {
    pub(crate) fn new(sender: mpsc::Sender<Command>) -> Self {
        NodeClient { sender }
    }

    pub async fn dial(&mut self, address: Multiaddr) -> Result<(), Box<dyn Error + Send>> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(Command::Dial { address, resp: tx })
            .await
            .expect("Command receiver not to be dropped.");
        rx.await.expect("Sender not to be dropped.")
    }

    pub async fn send_request(
        &mut self,
        dest: Multiaddr,
        payload: String,
    ) -> Result<String, Box<dyn Error + Send>> {
        let (tx, rx) = oneshot::channel();
        self.sender
            .send(Command::SendRequest {
                dest,
                payload,
                resp: tx,
            })
            .await
            .expect("Command receiver not to be dropped.");
        rx.await.expect("Sender not to be dropped.")
    }
}
