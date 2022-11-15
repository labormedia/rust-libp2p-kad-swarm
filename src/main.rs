use libp2p::identity::Keypair;
// use libp2p::core;
use libp2p::kad::{
    record::store::MemoryStore,
    Kademlia,
    KademliaConfig
};
use libp2p::swarm::NetworkBehaviour;
use libp2p::{
    identify,
    ping,
    relay::v2 as relay,
    swarm,
    NetworkBehaviour,
    Swarm,
    PeerId
};


pub struct LookupClient {
    local_key: Keypair,
    local_peer_id: PeerId,
    behaviour: LookupBehaviour,
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

#[derive(Debug, Clone)]
enum Network {
    Kusama
}

impl Network {
    fn protocol(&self) -> Option<String> {
        match self {
            Network::Kusama => Some("/ksmcc3/kad".to_string()),
        }
    }
}

impl LookupClient {
    fn new(self: &mut Self) -> &Self {
        self.set_local_key();
        self.set_peer_id();
        self
    }

    fn set_peer_id(self: &mut Self) {
        self.local_peer_id = PeerId::from(&self.local_key.public());
    }
    fn set_local_key(self: &mut Self) {
        self.local_key = Keypair::generate_ed25519();
    }
    fn set_behaviour(self: &mut Self, network: Option<Network>) {

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

        let (relay_transport, relay_client) = relay::client::Client::new_transport_and_behaviour(self.local_peer_id);

        self.behaviour = LookupBehaviour {
            kademlia,
            ping,
            identify,
            relay: relay_client,
            keep_alive: swarm::keep_alive::Behaviour,
        };
        // self.behaviour
    }
}


fn main() {
    println!("Hello, world!");
}
