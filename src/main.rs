use std::io;
use futures::executor::block_on;
use std::str::FromStr;
use std::time::Duration;
use libp2p::identity::Keypair;
// use libp2p::core;
use libp2p::kad::{
    record::store::MemoryStore,
    Kademlia,
    KademliaConfig
};
use libp2p::relay::v2::client::transport::ClientTransport;
use libp2p::{
    identify,
    ping,
    relay::v2 as relay,
    swarm::{
        self,
        SwarmBuilder,
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


pub struct LookupClient {
    local_key: Keypair,
    local_peer_id: PeerId,
    behaviour: LookupBehaviour,
    relay: relay::client::Client,
    // transport: OrTransport<ClientTransport, GenDnsConfig<GenTcpTransport<Tcp>, GenericConnection, GenericConnectionProvider<AsyncStdRuntime>>>,
    swarm: Swarm<LookupBehaviour>
}

#[derive(NetworkBehaviour)]
struct LookupBehaviour {
    pub(crate) kademlia: Kademlia<MemoryStore>,
    pub(crate) ping: ping::Behaviour,
    pub(crate) identify: identify::Behaviour,
    keep_alive: swarm::keep_alive::Behaviour,
}

#[derive(Debug, Clone)]
enum Network {
    Kusama
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
        vec![
            ("/dns/p2p.cc3-0.kusama.network/tcp/30100".parse().unwrap(), FromStr::from_str("12D3KooWDgtynm4S9M3m6ZZhXYu2RrWKdvkCSScc25xKDVSg1Sjd").unwrap()),
            ("/dns/p2p.cc3-1.kusama.network/tcp/30100".parse().unwrap(), FromStr::from_str("12D3KooWNpGriWPmf621Lza9UWU9eLLBdCFaErf6d4HSK7Bcqnv4").unwrap()),
            ("/dns/p2p.cc3-2.kusama.network/tcp/30100".parse().unwrap(), FromStr::from_str("12D3KooWLmLiB4AenmN2g2mHbhNXbUcNiGi99sAkSk1kAQedp8uE").unwrap()),
            ("/dns/p2p.cc3-3.kusama.network/tcp/30100".parse().unwrap(), FromStr::from_str("12D3KooWEGHw84b4hfvXEfyq4XWEmWCbRGuHMHQMpby4BAtZ4xJf").unwrap()),
            ("/dns/p2p.cc3-4.kusama.network/tcp/30100".parse().unwrap(), FromStr::from_str("12D3KooWF9KDPRMN8WpeyXhEeURZGP8Dmo7go1tDqi7hTYpxV9uW").unwrap()),
            ("/dns/p2p.cc3-5.kusama.network/tcp/30100".parse().unwrap(), FromStr::from_str("12D3KooWDiwMeqzvgWNreS9sV1HW3pZv1PA7QGA7HUCo7FzN5gcA").unwrap()),
            ("/dns/kusama-bootnode-0.paritytech.net/tcp/30333".parse().unwrap(), FromStr::from_str("12D3KooWSueCPH3puP2PcvqPJdNaDNF3jMZjtJtDiSy35pWrbt5h").unwrap()),
            ("/dns/kusama-bootnode-1.paritytech.net/tcp/30333".parse().unwrap(), FromStr::from_str("12D3KooWQKqane1SqWJNWMQkbia9qiMWXkcHtAdfW5eVF8hbwEDw").unwrap())
        ]
        
    }
    fn protocol(&self) -> Option<String> {
        match self {
            Network::Kusama => Some("/ksmcc3/kad".to_string()),
        }
    }
}

impl LookupClient {
    fn new(self: &mut Self, network: Option<Network>) -> &Self {
        self.set_local_key();
        self.set_peer_id();
        let transport = self.get_transport();
        let behaviour = self.get_behaviour(Some(Network::Kusama));
        self.set_swarm(Some(Network::Kusama), transport, behaviour);
        self

    }

    fn set_swarm(self: &mut Self, network: Option<Network>, transport: Boxed<(PeerId, StreamMuxerBox)>,behaviour: LookupBehaviour) -> &Self {
        let mut swarm = SwarmBuilder::new(transport, behaviour, self.local_peer_id)
        .executor(Box::new(|fut| {
            // async_std::task::spawn(fut);
        }))
        .build();

        if let Some(network) = network {
            for (addr, peer_id) in network.bootnodes() {
                swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
            }
        }
        self.swarm = swarm;
        self
    }

    fn set_peer_id(self: &mut Self) {
        self.local_peer_id = PeerId::from(&self.local_key.public());
    }
    fn set_local_key(self: &mut Self) {
        self.local_key = Keypair::generate_ed25519();
    }
    fn get_transport(self: &mut Self) -> Boxed<(PeerId, core::muxing::StreamMuxerBox)> {
        let (relay_transport, relay_client) = relay::client::Client::new_transport_and_behaviour(self.local_peer_id);
        self.relay = relay_client;
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
                        .into_authentic(&self.local_key)
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
                    .timeout(Duration::from_secs(20))
                    .map_err(|err| io::Error::new(io::ErrorKind::Other, err))
                    .boxed()
    }
    fn get_behaviour(self: &Self, network: Option<Network>) -> LookupBehaviour {

        // Create a Kademlia behaviour.
        let store = MemoryStore::new(self.local_peer_id);
        let mut kademlia_config = KademliaConfig::default();
        if let Some(protocol_name) = network.clone().map(|n| n.protocol()).flatten() {
            kademlia_config.set_protocol_names(vec![protocol_name.into_bytes().into()]);
        }
        let kademlia = Kademlia::with_config(self.local_peer_id, store, kademlia_config);

        let ping = ping::Behaviour::new(ping::Config::new());

        let user_agent =
            "substrate-node/v2.0.0-e3245d49d-x86_64-linux-gnu (unknown)".to_string();
        let proto_version = "/substrate/1.0".to_string();
        let identify = identify::Behaviour::new(
            identify::Config::new(proto_version, self.local_key.public())
                .with_agent_version(user_agent),
        );

        // self.behaviour = LookupBehaviour {
        //     kademlia,
        //     ping,
        //     identify,
        //     keep_alive: swarm::keep_alive::Behaviour,
        // };
        // self
        LookupBehaviour {
            kademlia,
            ping,
            identify,
            keep_alive: swarm::keep_alive::Behaviour,
        }
    }
}


fn main() {
    println!("Hello, world!");
}
