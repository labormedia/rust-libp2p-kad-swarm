use std::borrow::{BorrowMut};
use std::io;
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


pub struct LookupClient {
    local_key: Keypair,
    local_peer_id: PeerId,
    network: Vec<Network>,
    swarm: Swarm<LookupBehaviour>
}

#[derive(NetworkBehaviour)]
struct LookupBehaviour {
    pub(crate) kademlia: Kademlia<MemoryStore>,
    pub(crate) ping: ping::Behaviour,
    pub(crate) identify: identify::Behaviour,
    relay: relay::client::Client,
    keep_alive: swarm::keep_alive::Behaviour,
}

struct Peer {
    peer_id: PeerId,
    protocol_version: String,
    agent_version: String,
    listen_addrs: Vec<Multiaddr>,
    protocols: Vec<String>,
    observed_addr: Multiaddr,
}

#[derive(Debug, Clone)]
enum Network {
    Kusama
}

#[derive(Debug, Error)]
enum NetworkError {
    #[error("Request Timeout")]
    Timeout,
    #[error("Dial failed")]
    Dial,
    #[error("Resource not found")]
    NotFound,
    #[error("No Peers")]
    NoPeers,
}


impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "kusama" => Ok(Self::Kusama),
            n => Err(format!("Network '{}' not supported.", n)),
        }
    }
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
    fn new(net: Network) -> Self {
        // let mut pkcs8_der = std::fs::read("../rsa-keys-with-openssl/private.pk8").unwrap();
        let base_64_encoded = "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=";
        let expected_peer_id =
            PeerId::from_str("12D3KooWEChVMMMzV8acJ53mJHrw1pQ27UAGkCxWXLJutbeUMvVu").unwrap();

        let encoded = base64::decode(base_64_encoded).unwrap();

        let local_key = Keypair::from_protobuf_encoding(&encoded).unwrap();
        // let local_key = Keypair::from_pkcs8(&mut pkcs8_der).unwrap();
        // let local_key = Keypair::generate_ed25519();
        let local_peer_id = PeerId::from(local_key.public());
        let (relay_transport, relay_client) = relay::client::Client::new_transport_and_behaviour(local_peer_id);
        let transport = Self::get_transport(&local_key, &local_peer_id, relay_transport);
        let behaviour = Self::get_behaviour(&local_key, &local_peer_id, Some(&net), relay_client);
        let mut swarm = Self::build_swarm(local_peer_id, Some(Network::Kusama), transport, behaviour);
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap()).unwrap();
        let network = Vec::from([net]);
        LookupClient {
            local_key: local_key,
            local_peer_id: local_peer_id,
            network: network,
            swarm: swarm,
        }
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
    fn get_transport(local_key: &Keypair, local_peer_id: &PeerId, relay_transport: ClientTransport) -> Boxed<(PeerId, core::muxing::StreamMuxerBox)> {
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
                .into_authentic(&local_key)
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
    fn get_behaviour(local_key: &Keypair, local_peer_id: &PeerId, network: Option<&Network>, relay_client: Client) -> LookupBehaviour {
        let peer_id = local_peer_id.clone();
        // Create a Kademlia behaviour.
        let store = MemoryStore::new(peer_id);
        let mut kademlia_config = KademliaConfig::default();
        if let Some(protocol_name) = network.clone().map(|n| n.protocol()).flatten() {
            kademlia_config.set_protocol_names(vec![protocol_name.into_bytes().into()]);
        }
        let kademlia = Kademlia::with_config(peer_id, store, kademlia_config);

        let ping = ping::Behaviour::new(ping::Config::new());

        let user_agent =
            "substrate-node/v2.0.0-e3245d49d-x86_64-linux-gnu (unknown)".to_string();
        let proto_version = "/substrate/1.0".to_string();
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
    async fn identify(self: &mut Self, peer: PeerId) -> Result<Peer, NetworkError> {
        loop {
            if let SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(
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
            )) = self.swarm.next().await.expect("Infinite Stream.")
            {
                if peer_id == peer {
                    return Ok(Peer {
                        peer_id,
                        protocol_version,
                        agent_version,
                        listen_addrs,
                        protocols,
                        observed_addr,
                    });
                }
            }
        }
    }
    async fn dht(&mut self, peer: PeerId) -> Result<Peer, NetworkError> {
        type DynFuture = Box<dyn futures::future::Future<Output = Result<Peer, NetworkError>>>;
        self.swarm.behaviour_mut().kademlia.get_closest_peers(peer);
        loop {
            match self.swarm.next().await.expect("Infinite Stream.") {
                SwarmEvent::ConnectionEstablished {
                    peer_id,
                    num_established,
                    ..
                } => {
                    println!("Connection established {:?}", peer_id);
                    assert_ne!(Into::<u32>::into(num_established), 0);
                    if peer_id == peer {
                        self.swarm.behaviour_mut().kademlia.borrow_mut().addresses_of_peer(&peer);
                        return self.identify(peer).await;
                    }
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
                                return self.identify(peer).await
                            } else {                                
                                println!("Adding {:?} to kademlia addresses list.", addr);
                                if let Ok(p) = self.identify(addr).await {
                                    println!("{:?}", &p.observed_addr);
                                    self.swarm.behaviour_mut().kademlia.borrow_mut().add_address(&p.peer_id, p.observed_addr);
                                }
                                return Err(NetworkError::Timeout)
                            }
                            
                        };
                    }
                    return Err(NetworkError::NoPeers) 
                },
                SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {:?}", address),
                _ => {},
            }
        }

    }
    fn dial(self: &mut Self, peer: &Peer) -> Result<(), swarm::DialError> {
        println!("Dialing...");
        self.swarm.dial(peer.peer_id)
    }
    fn is_connected(self: &Self, peer: &Peer) -> bool {
        if Swarm::is_connected(&self.swarm, &peer.peer_id) { true } else { false }
    }
}

#[async_std::main]
async fn main() {
    println!("Starting Session");
    let mut lookup = LookupClient::new(Network::Kusama);
    let peer_query = PeerId::from_str("12D3KooWRtUUpNzH56YT8wWYoCJoTP1FH2Kq2CCY8PYxcHG1XjUc").unwrap();
    let b = if let Ok(a) = lookup.dht(peer_query).await {
        if lookup.is_connected(&a) {
            println!("{:?} seems connected. Dialing.", &a.peer_id);
            // let dialed = lookup.dial(&a).unwrap();
            // println!("Dialed : {:?}", dialed);
        } else {
            println!("Peer not connected. Dial suspended.")
        }
        a
    } else {
        println!("Repeating query");
        lookup.dht(peer_query).await.unwrap()
    };

    println!("Found {:?} {:?}", &b.peer_id, &b.listen_addrs);

    println!("Ending Session.")
}
