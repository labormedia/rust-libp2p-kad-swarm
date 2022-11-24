
use rust_libp2p_kad_swarm::*;
use libp2p::{core::PeerId, Multiaddr};
use std::str::FromStr;

#[async_std::main]
async fn main() {

    println!("Starting Session");
    // let a = "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=";
    // let mut lookup = LookupClient::from_base64(
    //     a, 
    //     &Network::Kusama
    // );
    let mut lookup = LookupClient::new(&Network::Kusama);
    let _ = lookup.listen().await ;
    let peer_query = PeerId::from_str("12D3KooWA8hDwAwVJZepYZ7NUrcz3deywN8gbxWSfCymXDVFBPKw").unwrap();  // known polkadot node example "12D3KooWDxhWkQ1LYMPcwUpcb7yy272DrMvGUoXH4wjkgzDXdu3d"
    // lookup.kademlia_add_address(peer_query, Multiaddr::from_str("/ip4/127.0.0.1/tcp/35691").unwrap()).await;

    let peer = match lookup.dht_query(peer_query).await {
        Ok(peer) => peer,
        Err(e) => panic!("{:?}",e)
        
    };
    println!("Found {:?} {:?} {:?}", peer.peer_id, peer.listen_addrs, peer.protocols);
    lookup.dial(&peer).await;

    println!("Ending Session.");

}

