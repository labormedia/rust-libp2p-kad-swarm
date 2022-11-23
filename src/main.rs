
use rust_libp2p_kad_swarm::*;
use libp2p::core::PeerId;
use std::str::FromStr;

#[async_std::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    println!("Starting Session");
    let mut lookup = LookupClient::from_base64(
        "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=", 
        &Network::Kusama
    );
    lookup.listen().await ;
    let peer_query = PeerId::from_str("12D3KooWDxhWkQ1LYMPcwUpcb7yy272DrMvGUoXH4wjkgzDXdu3d").unwrap();
    let a = match lookup.dht(peer_query).await {
        Ok(peer) => {
            if lookup.is_connected(&peer.peer_id) {
                println!("{:?} seems connected.", &peer.peer_id);
            } else {
                println!("Peer not connected.")
            }
            Ok(peer)
        }
        Err(e) => {
            println!("{:?} Repeating query...",e);
            lookup.dht(peer_query).await
        }
    };

    let b = match a {
        Ok(peer) => peer,
        Err(e) => panic!("{:?}",e)
        
    };
    println!("Found {:?} {:?} {:?}", b.peer_id, b.listen_addrs, b.protocols);
    lookup.dial(&b).await;


    println!("Ending Session.");
}

