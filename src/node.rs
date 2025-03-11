use std::{error::Error, time::Duration};

use libp2p::{
    Multiaddr, PeerId, StreamProtocol, Swarm,
    futures::{
        StreamExt,
        channel::mpsc::{self, Sender},
    },
    identify, identity, kad,
    multiaddr::Protocol,
    noise, request_response,
    swarm::NetworkBehaviour,
    tcp, yamux,
};
use serde::{Deserialize, Serialize};

pub struct Node {
    swarm: Swarm<Behaviour>,
}

pub enum Command {
    Dial(Multiaddr),
    SendRequest(Request),
}

impl Node {
    pub fn spawn(port: u32, seed: u8) -> Result<Sender<Command>, Box<dyn Error>> {
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

        let address = format!("/ip4/127.0.0.1/tcp/{port}").parse()?;
        swarm.listen_on(address)?;

        let (op_sender, mut op_receiver) = mpsc::channel(0);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    command = op_receiver.next() => match command {
                            Some(Command::Dial(addr)) => {
                                swarm.dial(addr).unwrap();
                            }
                            Some(Command::SendRequest(req)) => {
                                let dest = req.dest.clone();
                                let Some(Protocol::P2p(peer_id)) = dest.iter().last() else {
                                    panic!("Expect peer multiaddr to contain peer ID.");
                                };
                                swarm
                                    .behaviour_mut()
                                    .request_response
                                    .send_request(&peer_id , req);
                            }
                            None => break,
                    },
                    swarm_event = swarm.select_next_some() => match swarm_event {
                        _ => {}
                    }
                }
            }
        });

        Ok(op_sender)
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
