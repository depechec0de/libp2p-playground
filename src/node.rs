use core::fmt;
use std::{collections::HashMap, error::Error, time::Duration};

use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm,
    futures::{
        StreamExt,
        channel::mpsc::{self},
    },
    identify, identity, kad,
    multiaddr::Protocol,
    noise,
    request_response::{self, OutboundRequestId},
    swarm::{NetworkBehaviour, SwarmEvent},
    tcp, yamux,
};
use serde::{Deserialize, Serialize};

use crate::{ClientResponder, Command, NodeClient};

pub struct Node {
    swarm: Swarm<Behaviour>,
    pending_dials: HashMap<PeerId, ClientResponder<()>>,
    pending_requests: HashMap<OutboundRequestId, ClientResponder<String>>,
}

impl Node {
    pub fn spawn(port: u32, seed: u8) -> Result<NodeClient, Box<dyn Error>> {
        let mut bytes = [0u8; 32];
        bytes[0] = seed;
        let id_keys = identity::Keypair::ed25519_from_bytes(bytes).unwrap();

        let peer_id = id_keys.public().to_peer_id();

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
            .with_tokio()
            .with_tcp(
                tcp::Config::default().nodelay(true),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| Behaviour {
                request_response: request_response::cbor::Behaviour::<Request, Response>::new(
                    [(
                        StreamProtocol::new("/qaat/1.0.0"),
                        request_response::ProtocolSupport::Full,
                    )],
                    request_response::Config::default(),
                ),
                identify: identify::Behaviour::new(identify::Config::new(
                    "/qaat/id/1.0.0".to_string(),
                    key.public(),
                )),
                //relay: relay::Behaviour::new(peer_id, Default::default()),
                kademlia: kad::Behaviour::new(
                    key.public().to_peer_id(),
                    kad::store::MemoryStore::new(peer_id),
                ),
                //ping: ping::Behaviour::new(ping::Config::new()),
            })?
            .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(25)))
            .build();

        swarm
            .behaviour_mut()
            .kademlia
            .set_mode(Some(kad::Mode::Server));

        let listen_address = format!("/ip4/127.0.0.1/tcp/{port}").parse()?;
        swarm.listen_on(listen_address)?;

        let (op_sender, mut op_receiver) = mpsc::channel(0);

        let mut node = Node {
            swarm,
            pending_dials: Default::default(),
            pending_requests: Default::default(),
        };

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    command = op_receiver.next() => match command {
                            Some(Command::Dial { address, resp }) => {
                                if let Some(Protocol::P2p(peer_id)) = address.iter().last() {
                                    node.swarm.dial(address).unwrap();
                                    node.pending_dials.insert(peer_id, resp);
                                    tracing::info!(
                                        "Peer {:?} DIALING",
                                        node.swarm.local_peer_id(),
                                    );
                                } else {
                                    resp.send(Err(Box::new(NodeError::DialError))).expect("Receiver not to be dropped.");
                                };
                            }
                            Some(Command::SendRequest { dest, payload, resp }) => {
                                if let Some(Protocol::P2p(peer_id)) = dest.iter().last() {
                                    let request_id = node.swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_request(&peer_id , Request { dest, payload });

                                    node.pending_requests.insert(request_id, resp);
                                } else {
                                    resp.send(Err(Box::new(NodeError::SendRequestError))).expect("Receiver not to be dropped.");
                                };

                            }
                            None => break,
                    },
                    swarm_event = node.swarm.select_next_some() => match swarm_event {
                        SwarmEvent::ConnectionEstablished {peer_id, endpoint, .. } => {
                            if endpoint.is_dialer() {
                                if let Some(resp) = node.pending_dials.remove(&peer_id) {
                                    tracing::info!(
                                        "Peer {:?} CONNECTED AND RESPONDING {:?}",
                                        node.swarm.local_peer_id(),
                                        endpoint
                                    );
                                    let _ = resp.send(Ok(()));
                                }
                            }else{
                                tracing::info!(
                                    "Peer {:?} CONNECTED AS LISTENER {:?}",
                                    node.swarm.local_peer_id(),
                                    endpoint
                                );
                            }
                        }
                        SwarmEvent::Behaviour(BehaviourEvent::Identify(e)) => match e {
                            identify::Event::Received {
                                connection_id: _,
                                peer_id,
                                info,
                            } => {
                                info.listen_addrs.iter().for_each(|addr| {
                                    node.swarm
                                        .behaviour_mut()
                                        .kademlia
                                        .add_address(&peer_id, addr.clone());
                                });
                            }
                            event => {
                                tracing::debug!(
                                    "Peer {:?} SwarmEvent::Identify {:?}",
                                    node.swarm.local_peer_id(),
                                    event
                                );
                            }
                        },
                        SwarmEvent::Behaviour(BehaviourEvent::RequestResponse(
                            request_response::Event::Message {
                                message,
                                peer: sender_peer_id,
                            },
                        )) => {
                            match message {
                                request_response::Message::Request {
                                    request, channel, ..
                                } => {
                                    let response = Response {
                                        payload: format!("Bye!"),
                                    };
                                    node.swarm.behaviour_mut().request_response.send_response(channel, response).unwrap();

                                }
                                request_response::Message::Response {
                                    request_id,
                                    response,
                                } => {
                                    if let Some(resp) = node.pending_requests.remove(&request_id) {
                                        let _ = resp.send(Ok(response.payload));
                                    }
                                }
                            }
                        }
                        event => {
                            tracing::debug!(
                                "Peer {:?} SwarmEvent {:?}",
                                node.swarm.local_peer_id(),
                                event
                            );
                        }
                    }
                }
            }
        });

        Ok(NodeClient::new(op_sender))
    }
}

#[derive(NetworkBehaviour)]
struct Behaviour {
    request_response: request_response::cbor::Behaviour<Request, Response>,
    identify: identify::Behaviour,
    //relay: relay::Behaviour,
    kademlia: kad::Behaviour<kad::store::MemoryStore>,
    //ping: ping::Behaviour,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Request {
    pub dest: Multiaddr,
    pub payload: String,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Response {
    payload: String,
}

#[derive(Debug)]
enum NodeError {
    SendRequestError,
    DialError,
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeError::SendRequestError => write!(f, "Error sending the request"),
            NodeError::DialError => write!(f, "Error dialing"),
        }
    }
}

impl Error for NodeError {}
