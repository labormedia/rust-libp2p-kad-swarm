// Example usage for listening a Request and emit a Response.

use rust_libp2p_kad_swarm as synack_node;
use libp2p::core::PeerId;
use std::str::FromStr;

#[async_std::main]
async fn main() {
    // If you want to fix your local PeerId, an alternative is to use a base64 protobuf encoding of the public key.
    // let mut a = synack_node::LookupClient::from_base64(
    //     "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=", 
    //     &synack_node::Network::Kusama
    // );
    let mut a = synack_node::LookupClient::new(
        &synack_node::Network::Kusama
    );
    let _ = a.listen().await;
    // Make a query to a previously known address bootnode to traverse the kademlia dht ephemereal network.
    let peer = match a.dht_query(PeerId::from_str("12D3KooWDxhWkQ1LYMPcwUpcb7yy272DrMvGUoXH4wjkgzDXdu3d").unwrap()).await {
        Ok(peer) => peer,
        Err(e) => panic!("{:?}",e)
        
    };
    println!("Found {:?} {:?} {:?}", peer.peer_id, peer.listen_addrs, peer.protocols);
    println!("Observed peer_id and addresses : {:?} {:?}", a.local_peer_id, peer.observed_addr);
    let request_peer_address = a.init_protocol().await;
    println!("SYN from node[{:?}]", request_peer_address);
}