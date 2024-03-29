use std::borrow::{BorrowMut};
use std::io;
use futures::{
    executor::block_on,
    stream::{
        StreamExt,
    },
};
use libp2p::relay::v2::client::Client;
use libp2p::request_response::{RequestResponseCodec, RequestResponse};
use std::time::Duration;
use libp2p_core::{
    self,
    transport::{
        OrTransport,
        Transport,
        Boxed
    },
    upgrade::{
        self,
        InboundUpgradeExt,
        OutboundUpgradeExt
    },
    identity::Keypair,
    PeerId
};
use libp2p_kad::{
    record::store::MemoryStore,
    Kademlia,
    KademliaConfig,
    KademliaEvent,
    QueryResult,
    GetClosestPeersOk
};
use libp2p::swarm::{
    Swarm,
    SwarmBuilder,
    SwarmEvent,
    NetworkBehaviour
};
use libp2p::relay::v2::client::transport::ClientTransport;
use libp2p::{
    identify,
    ping,
    relay::v2 as relay,
    Multiaddr,
    noise,
    mplex,
    yamux,
    dns,
    tcp,
};
use libp2p_quic as quic;
use libp2p_core::muxing::StreamMuxerBox;
use thiserror::Error;
use std::str::FromStr;
#[cfg(feature = "request-response")]
use libp2p::request_response;
#[cfg(feature = "test-protocol")]
use std::iter;

#[derive(libp2p_swarm::NetworkBehaviour)]
pub struct LookupBehaviour {
    pub(crate) kademlia: Kademlia<MemoryStore>,
    pub(crate) ping: ping::Behaviour,
    pub(crate) identify: identify::Behaviour,
    #[cfg(feature = "test-protocol")]
    pub request_response: RequestResponse<TestCodec>,
    relay: relay::client::Client,
    keep_alive: libp2p_swarm::keep_alive::Behaviour,
}

pub struct LookupClient {
    // local_key: Keypair,
    pub local_peer_id: PeerId,
    pub listen_addrs: Vec<Multiaddr>,
    pub network: Vec<Network>,
    pub swarm: Swarm<LookupBehaviour>
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
        let local_peer_id = local_key.public().to_peer_id();
        println!("Local PeerID : {:?}", local_peer_id);
        let (relay_transport, relay_client) = relay::client::Client::new_transport_and_behaviour(local_peer_id);
        let transport = Self::build_transport(&local_key, relay_transport);
        let behaviour = Self::build_behaviour(&local_key, &local_peer_id, Some(net), relay_client);
        let swarm = Self::build_swarm(local_peer_id, Some(net.clone()), transport, behaviour);
        let network = Vec::from([net.clone()]);
        let listen_addrs: Vec<Multiaddr> = [].to_vec();
        LookupClient {
            // local_key,
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
    fn build_transport(local_key: &Keypair, relay_transport: ClientTransport) -> Boxed<(PeerId, libp2p_core::muxing::StreamMuxerBox)> {

        let mut config = quic::Config::new(local_key);
        // config.handshake_timeout = Duration::from_secs(1);
    
        // let quic_transport = quic::async_std::Transport::new(config);

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

            upgrade::SelectUpgrade::new(yamux_config, mplex_config)
                .map_inbound(libp2p_core::muxing::StreamMuxerBox::new)
                .map_outbound(libp2p_core::muxing::StreamMuxerBox::new)
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

        #[cfg(feature = "test-protocol")]
        let synack_protocol = RequestResponse::new(
            TestCodec(), iter::once((TestProtocol(), 
            request_response::ProtocolSupport::Full)), 
            request_response::RequestResponseConfig::default() 
        );

        let user_agent =
            "substrate-node/v2.0.0-e3245d49d-x86_64-linux-gnu (unknown)".to_string();
        let proto_version = "/ipfs/id/1.0.0".to_string();
        let identify = identify::Behaviour::new(
            identify::Config::new(proto_version, local_key.public())
                .with_agent_version(user_agent),
        );

        LookupBehaviour {
            kademlia,
            ping,
            identify,
            #[cfg(feature = "test-protocol")]
            request_response: synack_protocol,
            relay: relay_client,
            keep_alive: libp2p_swarm::keep_alive::Behaviour,
        }
    }
    pub async fn listen(&mut self) -> Result<libp2p_core::transport::ListenerId, libp2p::TransportError<io::Error>> {
        self.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())
    }
    async fn dht(&mut self, peer: PeerId) -> Result<Peer, NetworkError> {
        // type DynFuture = Box<dyn futures::future::Future<Output = Result<Peer, NetworkError>>>;
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
                        let listen_addrs = addr.listen_addrs[0].clone(); // We are asumming the first address is available and accesible.
                        self.swarm.behaviour_mut().kademlia.borrow_mut().add_address(&addr.peer_id,listen_addrs );
                    }
                },
                SwarmEvent::Behaviour(LookupBehaviourEvent::Kademlia(
                    KademliaEvent::RoutingUpdated { 
                        peer, 
                        is_new_peer: _, 
                        addresses: _, 
                        bucket_range: _, 
                        old_peer: _ 
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
    pub async fn dial(&mut self, peer_to_dial: &Peer) {
        let address_to_dial = peer_to_dial.listen_addrs[0].clone();
        println!("Dialing...{:?}", address_to_dial);
        self.swarm.dial(address_to_dial).unwrap()
    }
    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        Swarm::is_connected(&self.swarm, peer_id)
    }
    #[cfg(feature="test-protocol")]
    pub async fn send_request(&mut self, peer_id:PeerId, payload: test_protocol::SYN) {
        self.swarm.behaviour_mut().request_response.send_request(&peer_id, payload);
    }
    #[cfg(feature="test-protocol")]
    pub async fn send_response(&mut self, channel: ResponseChannel<test_protocol::SYNACK>, payload: test_protocol::SYNACK) {
        self.swarm.behaviour_mut().request_response.send_response(channel, payload).unwrap();
    }
    pub async fn kademlia_add_address(&mut self, peer_id: PeerId, address: Multiaddr) {
        self.swarm.behaviour_mut().kademlia.borrow_mut().add_address(&peer_id, address);
    }
    #[cfg(feature="test-protocol")]
    pub async fn add_address(&mut self, peer_id: PeerId, address: Multiaddr) {
        self.swarm
            .behaviour_mut()
            .request_response
            .borrow_mut()
            .add_address(&peer_id, address)    
    }
    #[cfg(feature="test-protocol")]
    pub async fn init_protocol(&mut self) -> Result<PeerId,NetworkError> {

        let synack = test_protocol::SYNACK("SYNACK".to_string().into_bytes());
        let ack = test_protocol::SYN("ACK".to_string().into_bytes());
        loop {
            match self.swarm.next().await.expect("Infinite Stream.") {
                SwarmEvent::NewListenAddr { address, .. } => { println!("New Listen Address : {:?}",address); },
                SwarmEvent::Behaviour( LookupBehaviourEvent::RequestResponse(
                    RequestResponseEvent::ResponseSent { peer, .. }
                ) ) => {
                    println!("Response sent to : {:?}", peer);
                    break Ok(peer)
                },
                SwarmEvent::Behaviour(
                    LookupBehaviourEvent::RequestResponse (
                        RequestResponseEvent::Message { 
                            peer, 
                            message: 
                                RequestResponseMessage::Response { 
                                    request_id, 
                                    response 
                                } }
                    )
                ) => {
                    match response {
                        test_protocol::SYNACK(payload) => { 
                            println!("Response received : {:?} {:?} {:?}", peer, request_id, std::str::from_utf8(&payload).unwrap());
                            self.send_request( peer, ack.clone()).await;
                            match std::str::from_utf8(&payload).unwrap() {
                                "ACK" => { 
                                     // TODO : timer
                                    break Ok(peer)
                                }
                                "SYNACK" => {
                                    // self.send_request( peer, ack.clone()).await;
                                    println!("Handshake succeeded.");
                                    break Ok(peer)
                                }
                                _ => {}
                            }
                        }
                    }
                },
                SwarmEvent::Behaviour( LookupBehaviourEvent::RequestResponse(
                    RequestResponseEvent::Message {
                        peer,
                        message:
                            RequestResponseMessage::Request {
                                request, 
                                channel, ..
                            },
                    }
                    )
                ) => {
                    match request {
                        test_protocol::SYN(payload) => { 
                            println!("Request received from : {:?} {:?}", peer, std::str::from_utf8(&payload).unwrap());
                            match std::str::from_utf8(&payload).unwrap() {
                                "SYN" => {
                                    self.send_response(channel, synack.clone()).await;
                                },
                                "ACK" => {
                                    println!("Handshake succeeded.");
                                    break Ok(peer);
                                },
                                _ => {}
                            }
                        }
                    }
                },
                _ => { }
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use libp2p_swarm::DialError;

    use super::*;

    #[test]
    fn peerid_from_base64_string() {
        let lookup = LookupClient::from_base64(
            "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=", 
            &Network::Kusama
        );
        assert_eq!(lookup.local_peer_id, PeerId::from_str("12D3KooWEChVMMMzV8acJ53mJHrw1pQ27UAGkCxWXLJutbeUMvVu").unwrap())
    }

    #[async_std::test]
    async fn local_dial() -> Result<(), libp2p_swarm::DialError>{
        // the next address will be considered. 
        let addrs_count = 0; 
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
        // the next address will be considered. 
        let addrs_count = 0; 
        let net = Network::Kusama;
        let mut node_a = LookupClient::new(&net);
        let mut node_b = LookupClient::new(&net);
        let node_a = async_std::task::spawn(async move {
            let _ = node_a.listen().await;
            let addr = loop {
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
                            peer_id: _,
                            info:
                                identify::Info {
                                    protocol_version: _,
                                    agent_version: _,
                                    listen_addrs: _,
                                    protocols: _,
                                    observed_addr: _,
                                    ..
                                },
                        },
                    )) => { println!("Identified Received") }
                    SwarmEvent::NewListenAddr { address, .. } => {
                        println!("Listening on {:?}", address);
                        node_b.listen_addrs.push(address);
                    },
                    SwarmEvent::Behaviour(LookupBehaviourEvent::Identify(
                        identify::Event::Sent {
                            peer_id
                        },
                    )) => {
                        println!("Sent identify info to {:?}", peer_id);
                    }
                    SwarmEvent::Behaviour(LookupBehaviourEvent::Kademlia(
                        KademliaEvent::RoutingUpdated { 
                            peer, 
                            is_new_peer: _, 
                            addresses: _, 
                            bucket_range: _, 
                            old_peer: _ 
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




// Protocol dependencies .

use async_trait::async_trait;
use libp2p::request_response::*;
// use std::io;
use futures::{prelude::*, AsyncWriteExt};
use libp2p_core::upgrade::{
    read_length_prefixed,
    write_length_prefixed
};

#[derive(Debug, Clone)]
pub struct TestProtocol();
#[derive(Clone)]
pub struct TestCodec();
// The message types will be derived from the protocol types specification located at 'protocols/test-protocol/src/lib.rs'
// This is intented for workspace management of protocols.
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct SYN(Vec<u8>);
// #[derive(Debug, Clone, PartialEq, Eq)]
// pub struct SYNACK(Vec<u8>);
// #[derive(Debug, Clone, PartialEq, Eq)]
// struct ACK(Vec<u8>);

impl ProtocolName for TestProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/SYNACK/0.0.1".as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for TestCodec {
    type Protocol = TestProtocol;
    type Request = test_protocol::SYN;
    type Response = test_protocol::SYNACK;

    async fn read_request<T>(&mut self, _: &TestProtocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1024).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(test_protocol::SYN(vec))
    }

    async fn read_response<T>(&mut self, _: &TestProtocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1024).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(test_protocol::SYNACK(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &TestProtocol,
        io: &mut T,
        test_protocol::SYN(data): test_protocol::SYN,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &TestProtocol,
        io: &mut T,
        test_protocol::SYNACK(data): test_protocol::SYNACK,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }
}