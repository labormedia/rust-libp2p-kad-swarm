use std::borrow::{BorrowMut};
use std::{io, clone};
// use futures::{TryFutureExt, FutureExt};
use futures::{
    executor::block_on,
    stream::{
        StreamExt,
    },
};
use libp2p::relay::v2::client::Client;
use std::time::Duration;
use libp2p::identity::Keypair;
use libp2p::kad::{
    record::store::MemoryStore,
    Kademlia,
    KademliaConfig,
    KademliaEvent,
    QueryResult,
    GetClosestPeersOk
};
use libp2p::relay::v2::client::transport::ClientTransport;
use libp2p::{
    identify,
    ping,
    relay::v2 as relay,
    swarm::{
        self,
        SwarmBuilder,
        SwarmEvent,
        NetworkBehaviour
    },
    NetworkBehaviour,
    Swarm,
    PeerId,
    Multiaddr,
    noise,
    mplex,
    yamux,
    dns,
    tcp,
    InboundUpgradeExt,
    OutboundUpgradeExt
};
use libp2p::core;
use libp2p::core::{
    transport::{
        OrTransport,
        Transport,
        Boxed
    },
    upgrade
};
use crate::core::muxing::StreamMuxerBox;
use thiserror::Error;
use std::str::FromStr;
use libp2p::request_response::*;


pub struct LookupClient {
    local_key: Keypair,
    pub local_peer_id: PeerId,
    pub listen_addrs: Vec<Multiaddr>,
    pub network: Vec<Network>,
    swarm: Swarm<LookupBehaviour>
}

#[derive(NetworkBehaviour)]
pub struct LookupBehaviour {
    pub(crate) kademlia: Kademlia<MemoryStore>,
    pub(crate) ping: ping::Behaviour,
    pub(crate) identify: identify::Behaviour,
    relay: relay::client::Client,
    keep_alive: swarm::keep_alive::Behaviour,
}

pub struct Peer {
    pub peer_id: PeerId,
    pub protocol_version: String,
    pub agent_version: String,
    pub listen_addrs: Vec<Multiaddr>,
    pub protocols: Vec<String>,
    pub observed_addr: Multiaddr,
}

#[derive(Debug, Clone)]
pub enum Network {
    Kusama
}

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("Request Timeout")]
    Timeout,
    #[error("Dial failed")]
    DialError,
    #[error("Resource not found")]
    NotFound,
    #[error("No Peers")]
    NoPeers,
}

impl Network {
    fn bootnodes(&self) -> Vec<(Multiaddr, PeerId)> {
        match self {
            Network::Kusama => vec![
                ("/dns/p2p.cc3-0.kusama.network/tcp/30100".parse().unwrap(), PeerId::from_str("12D3KooWDgtynm4S9M3m6ZZhXYu2RrWKdvkCSScc25xKDVSg1Sjd").unwrap()),
                ("/dns/p2p.cc3-1.kusama.network/tcp/30100".parse().unwrap(), PeerId::from_str("12D3KooWNpGriWPmf621Lza9UWU9eLLBdCFaErf6d4HSK7Bcqnv4").unwrap()),
                ("/dns/p2p.cc3-2.kusama.network/tcp/30100".parse().unwrap(), PeerId::from_str("12D3KooWLmLiB4AenmN2g2mHbhNXbUcNiGi99sAkSk1kAQedp8uE").unwrap()),
                ("/dns/p2p.cc3-3.kusama.network/tcp/30100".parse().unwrap(), PeerId::from_str("12D3KooWEGHw84b4hfvXEfyq4XWEmWCbRGuHMHQMpby4BAtZ4xJf").unwrap()),
                ("/dns/p2p.cc3-4.kusama.network/tcp/30100".parse().unwrap(), PeerId::from_str("12D3KooWF9KDPRMN8WpeyXhEeURZGP8Dmo7go1tDqi7hTYpxV9uW").unwrap()),
                ("/dns/p2p.cc3-5.kusama.network/tcp/30100".parse().unwrap(), PeerId::from_str("12D3KooWDiwMeqzvgWNreS9sV1HW3pZv1PA7QGA7HUCo7FzN5gcA").unwrap()),
                ("/dns/kusama-bootnode-0.paritytech.net/tcp/30333".parse().unwrap(), PeerId::from_str("12D3KooWSueCPH3puP2PcvqPJdNaDNF3jMZjtJtDiSy35pWrbt5h").unwrap()),
                ("/dns/kusama-bootnode-1.paritytech.net/tcp/30333".parse().unwrap(), PeerId::from_str("12D3KooWQKqane1SqWJNWMQkbia9qiMWXkcHtAdfW5eVF8hbwEDw").unwrap())
            ]
        }        
    }
    
    fn protocol(&self) -> Option<String> {
        match self {
            Network::Kusama => Some("/ksmcc3/kad".to_string()),
        }
    }
}

impl LookupClient {
    fn builder(local_key: Keypair, net: &Network) -> Self {
        let local_peer_id = PeerId::from(local_key.public());
        println!("Local PeerID : {:?}", local_peer_id);
        let (relay_transport, relay_client) = relay::client::Client::new_transport_and_behaviour(local_peer_id);
        let transport = Self::build_transport(&local_key, relay_transport);
        let behaviour = Self::build_behaviour(&local_key, &local_peer_id, Some(&net), relay_client);
        let swarm = Self::build_swarm(local_peer_id, Some(net.clone()), transport, behaviour);
        let network = Vec::from([net.clone()]);
        let listen_addrs: Vec<Multiaddr> = [].to_vec();
        LookupClient {
            local_key,
            local_peer_id,
            listen_addrs,
            network,
            swarm,
        }
    }
    // TODO: trait implementations for multiple key sources.
    pub fn from_base64(base64_string: &str, net: &Network) -> Self {
        let encoded = base64::decode(base64_string).unwrap();
        Self::builder(Keypair::from_protobuf_encoding(&encoded).unwrap(), net)
    }
    pub fn from_pkcs8_file(file_path: &str, net: &Network) -> Self {
        let mut pkcs8_der = std::fs::read(file_path).unwrap();
        Self::builder(Keypair::rsa_from_pkcs8(&mut pkcs8_der).unwrap(), net)
    }
    pub fn new(net: &Network) -> Self {
        let local_key = Keypair::generate_ed25519();
        Self::builder(local_key, net)
    }

    fn build_swarm(local_peer_id: PeerId, network: Option<Network>, transport: Boxed<(PeerId, StreamMuxerBox)>,behaviour: LookupBehaviour) -> Swarm<LookupBehaviour> {
        let mut swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
        .executor(Box::new(|fut| {
            async_std::task::spawn(fut);
        }))
        .build();

        if let Some(network) = network {
            for (addr, peer_id) in network.bootnodes() {
                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
            }
        }
        swarm
    }
    fn build_transport(local_key: &Keypair, relay_transport: ClientTransport) -> Boxed<(PeerId, core::muxing::StreamMuxerBox)> {
        // Reference: https://github.com/mxinden/libp2p-lookup/blob/41f4e2fc498b44bcdd2d4b381363dea0b740336b/src/main.rs#L136-L175
        let transport = OrTransport::new(
            relay_transport,
            block_on(dns::DnsConfig::system(tcp::TcpTransport::new(
                tcp::GenTcpConfig::new().port_reuse(true).nodelay(true),
            )))
            .unwrap(),
        );

        let authentication_config = {
            let noise_keypair_spec = noise::Keypair::<noise::X25519Spec>::new()
                .into_authentic(local_key)
                .unwrap();

            noise::NoiseConfig::xx(noise_keypair_spec).into_authenticated()
        };

        let multiplexing_config = {
            let mut mplex_config = mplex::MplexConfig::new();
            mplex_config.set_max_buffer_behaviour(mplex::MaxBufferBehaviour::Block);
            mplex_config.set_max_buffer_size(usize::MAX);

            let mut yamux_config = yamux::YamuxConfig::default();
            // Enable proper flow-control: window updates are only sent when
            // buffered data has been consumed.
            yamux_config.set_window_update_mode(yamux::WindowUpdateMode::on_read());

            core::upgrade::SelectUpgrade::new(yamux_config, mplex_config)
                .map_inbound(core::muxing::StreamMuxerBox::new)
                .map_outbound(core::muxing::StreamMuxerBox::new)
        };

        transport
            .upgrade(upgrade::Version::V1)
            .authenticate(authentication_config)
            .multiplex(multiplexing_config)
            .timeout(Duration::from_secs(1000))
            .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
            .boxed()
    }
    fn build_behaviour(local_key: &Keypair, local_peer_id: &PeerId, network: Option<&Network>, relay_client: Client) -> LookupBehaviour {
        let peer_id = *local_peer_id;
        // Create a Kademlia behaviour.
        let store = MemoryStore::new(peer_id);
        let mut kademlia_config = KademliaConfig::default();
        if let Some(protocol_name) = network.and_then(|n| n.protocol()) {
            kademlia_config.set_protocol_names(vec![protocol_name.into_bytes().into()]);
        }
        let kademlia = Kademlia::with_config(peer_id, store, kademlia_config);

        let ping = ping::Behaviour::new(ping::Config::new());

        let user_agent =
            "substrate-node/v2.0.0-e3245d49d-x86_64-linux-gnu (unknown)".to_string();
        let proto_version = "/ipfs/id/1.0.0".to_string();
        // let proto_version = "/ipfs/id/1.0.0".to_string();
        let identify = identify::Behaviour::new(
            identify::Config::new(proto_version, local_key.public())
                .with_agent_version(user_agent),
        );

        LookupBehaviour {
            kademlia,
            ping,
            identify,
            relay: relay_client,
            keep_alive: swarm::keep_alive::Behaviour,
        }
    }
    pub async fn listen(self: &mut Self) -> Result<core::transport::ListenerId, libp2p::TransportError<io::Error>> {
        self.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
    }
    async fn dht(&mut self, peer: PeerId) -> Result<Peer, NetworkError> {
        type DynFuture = Box<dyn futures::future::Future<Output = Result<Peer, NetworkError>>>;
        self.swarm.behaviour_mut().kademlia.get_closest_peers(peer);
        loop {
            match self.swarm.next().await.expect("Infinite Stream.") {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening on {:?}", address);
                    self.listen_addrs.push(address);
                },
                SwarmEvent::ConnectionEstablished {
                    peer_id,
                    num_established,
                    ..
                } => {
                    println!("Connection established {:?}", peer_id);
                    assert_ne!(Into::<u32>::into(num_established), 0);
                    if peer_id == peer {
                        self.swarm.behaviour_mut().kademlia.borrow_mut().addresses_of_peer(&peer);
                    }
                },
                // SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(
                //     identify::Event::Sent {
                //         peer_id
                //     },
                // ))  => {
                //     println!("Sent identify info to {:?}", peer_id);
                //     // break peer_id;
                // },
                SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(
                    identify::Event::Received {
                        peer_id,
                        info:
                            identify::Info {
                                protocol_version,
                                agent_version,
                                listen_addrs,
                                protocols,
                                observed_addr,
                                ..
                            },
                    },
                )) => {
                    let addr = Peer {
                        peer_id,
                        protocol_version,
                        agent_version,
                        listen_addrs,
                        protocols,
                        observed_addr,
                    };
                    if peer_id == peer {
                        break Ok(addr);
                    } else {
                        println!("Adding {:?} to kademlia addresses list.", &addr.peer_id);
                        println!("Listened addresses : {:?}", &addr.listen_addrs);
                        let listen_addrs = addr.listen_addrs[0].clone();
                        self.swarm.behaviour_mut().kademlia.borrow_mut().add_address(&addr.peer_id,listen_addrs );
                    }
                },
                SwarmEvent::Behaviour(LookupBehaviourEvent::Kademlia(
                    KademliaEvent::RoutingUpdated { 
                        peer, 
                        is_new_peer, 
                        addresses, 
                        bucket_range, 
                        old_peer 
                    })) => {
                        println!("{:?} added in the Routing Table.", peer);
                },
                SwarmEvent::Behaviour(LookupBehaviourEvent::Kademlia(
                    KademliaEvent::OutboundQueryCompleted {
                        result: QueryResult::Bootstrap(_),
                        ..
                    },
                )) => {
                    panic!("Unexpected bootstrap.");
                },
                SwarmEvent::Behaviour(LookupBehaviourEvent::Kademlia(
                    KademliaEvent::OutboundQueryCompleted {
                        result: QueryResult::GetClosestPeers(Ok(GetClosestPeersOk { peers, .. })),
                        ..
                    },
                )) => {
                    let num_peers = &peers.len();
                    if num_peers > &0 {
                        for addr in peers {
                            if addr == peer {
                                println!("Eureka! {:?} ", addr);
                            }
                        };
                        return Err(NetworkError::Timeout) ;
                    }
                    return Err(NetworkError::NoPeers) 
                },
                _ => {}
            }
        }

    }
    pub async fn dht_query(&mut self, peer_query: PeerId) -> Result<Peer, NetworkError> {
        match self.dht(peer_query).await {
            Ok(peer) => {
                if self.is_connected(&peer.peer_id) {
                    println!("{:?} seems connected.", &peer.peer_id);
                } else {
                    println!("Peer not connected.")
                }
                Ok(peer)
            }
            Err(e) => {
                println!("{:?} Repeating query...",e);
                self.dht(peer_query).await
            }
        }
    }
    pub async fn dial(self: &mut Self, peer_to_dial: &Peer) -> () {
        let address_to_dial = peer_to_dial.listen_addrs[0].clone();
        println!("Dialing...{:?}", address_to_dial);
        self.swarm.dial(address_to_dial).unwrap()
    
        // Thinking if this method should have an event loop or not.
        // loop {
        //     match self.swarm.select_next_some().await {
        //         SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {:?}", address),
        //         // Prints peer id identify info is being sent to.
        //         SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(identify::Event::Sent { peer_id, .. }) ) => {
        //             println!("Sent identify info to {:?}", peer_id)
        //             // print!(".");
        //         }
        //         // Prints out the info received via the identify event
        //         SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(identify::Event::Received { peer_id, info, .. })) => {
        //             if peer_id == peer_to_dial.peer_id {
        //                 println!("Received {:?} {:?}", peer_id, info);
        //                 break Ok(peer_id);
        //             } else {
        //                 print!(".");
        //             }
        //         }
        //         _ => {}
        //     }
        // }
    }
    pub fn is_connected(self: &Self, peer_id: &PeerId) -> bool {
        Swarm::is_connected(&self.swarm, &peer_id)
    }
    pub async fn events(self: &mut Self) {

    }
}

#[cfg(test)]
mod tests {

    use libp2p::swarm::DialError;

    use super::*;

    #[test]
    fn peerid_from_base64_string() {
        let lookup = LookupClient::from_base64(
            // let base_64_encoded = "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=";
            // let expected_peer_id = PeerId::from_str("12D3KooWEChVMMMzV8acJ53mJHrw1pQ27UAGkCxWXLJutbeUMvVu").unwrap();
            "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=", 
            &Network::Kusama
        );
        assert_eq!(lookup.local_peer_id, PeerId::from_str("12D3KooWEChVMMMzV8acJ53mJHrw1pQ27UAGkCxWXLJutbeUMvVu").unwrap())
    }

    #[async_std::test]
    async fn local_dial() -> Result<(), swarm::DialError>{
        let addrs_count = 0; // change this if you want to test another address or you have fewer count of addresses
        let mut node_a = LookupClient::new(&Network::Kusama);
        let mut node_b = LookupClient::new(&Network::Kusama);
        let node_b = async_std::task::spawn(async move {
            let _ = node_b.listen().await;
            let addr = loop {
                if let SwarmEvent::NewListenAddr { address, .. } = node_b.swarm.select_next_some().await {

                    println!("Listening to address : {:?}", address);
                    node_b.listen_addrs.push(address);
                    if node_b.listen_addrs.len() > addrs_count {
                        break node_b;
                    }
                };
            };
            addr
        }).await;
        println!("Listening addresses : {:?}", node_b.listen_addrs);
        node_a.swarm.dial(node_b.listen_addrs[addrs_count].clone())
    }

    #[async_std::test]
    async fn async_identify() -> Result<(), DialError> {
        println!("again");
        let addrs_count = 0; // change this if you want to test another address or you have fewer count of addresses
        let net = Network::Kusama;
        let mut node_a = LookupClient::new(&net);
        let mut node_b = LookupClient::new(&net);
        // let transport = libp2p::development_transport(node_a.local_key).await?;

        // // Create a identify network behaviour.
        // let behaviour = identify::Behaviour::new(identify::Config::new(
        //     "/ipfs/id/1.0.0".to_string(),
        //     node_a.local_key.public(),
        // ));
  
        let mut node_a = async_std::task::spawn(async move {
            let _ = node_a.listen().await;
            let mut addr = loop {
                match node_a.swarm.select_next_some().await {
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {:?}", address);
                        node_a.listen_addrs.push(address);
                        if node_a.listen_addrs.len() > addrs_count {
                            break node_a;
                        }
                    },
                    SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(
                        identify::Event::Received {
                            peer_id,
                            info:
                                identify::Info {
                                    protocol_version,
                                    agent_version,
                                    listen_addrs,
                                    protocols,
                                    observed_addr,
                                    ..
                                },
                        },
                    )) =>  {
                        let addr = Peer {
                            peer_id,
                            protocol_version,
                            agent_version,
                            listen_addrs,
                            protocols,
                            observed_addr,
                        };
                        // if peer_id == node_b.local_peer_id {
                        //     println!("Found.");
                        // } else 
                        {
                            println!("Adding {:?} to kademlia addresses list.", &addr.peer_id);
                            println!("Listened addresses : {:?}", &addr.listen_addrs);
                            let listen_addrs = addr.listen_addrs[0].clone();
                            node_a.swarm.behaviour_mut().kademlia.borrow_mut().add_address(&addr.peer_id,listen_addrs );
                        }
                    },
                    _ => {},
                };

            };
            addr
        }).await;
        let result = async_std::task::spawn(async move {
            println!("Target addresses : {:?}", node_a.listen_addrs);
            let _ = node_b.listen().await;
            node_b.swarm.behaviour_mut().kademlia.borrow_mut().add_address(&node_a.local_peer_id,node_a.listen_addrs[addrs_count].clone() );
            loop {
                match node_b.swarm.select_next_some().await {
                    SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(
                        identify::Event::Received {
                            peer_id,
                            info:
                                identify::Info {
                                    protocol_version,
                                    agent_version,
                                    listen_addrs,
                                    protocols,
                                    observed_addr,
                                    ..
                                },
                        },
                    )) => { println!("Hello Event")}
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {:?}", address);
                        node_b.listen_addrs.push(address);
                        // if node_a.listen_addrs.len() > addrs_count {
                        //     break node_a;
                        // }
                        // break "TODO".to_string();
                    },
                    SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(
                        identify::Event::Sent {
                            peer_id
                        },
                    )) => {
                        println!("Sent identify info to {:?}", peer_id);
                        // break peer_id;
                    }
                    SwarmEvent::Behaviour(LookupBehaviourEvent::Kademlia(
                        KademliaEvent::RoutingUpdated { 
                            peer, 
                            is_new_peer, 
                            addresses, 
                            bucket_range, 
                            old_peer 
                        })) => {
                            println!("{:?} added in the Routing Table.", peer);
                            if peer == node_a.local_peer_id {
                                break node_b.swarm.dial(node_a.listen_addrs[addrs_count].clone());
                            } 
                    },
                    SwarmEvent::Behaviour(event) => println!("{event:?}"),
                    _ => {}
                }
            }
        }).await;
        result
    }
}