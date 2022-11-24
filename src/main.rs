
use rust_libp2p_kad_swarm::*;
use libp2p::core::PeerId;
use std::str::FromStr;

#[async_std::main]
async fn main() {

    println!("Starting Session");

    let mut lookup = LookupClient::new(&Network::Kusama);
    let _ = lookup.listen().await ;
    let peer_query = PeerId::from_str("12D3KooWDxhWkQ1LYMPcwUpcb7yy272DrMvGUoXH4wjkgzDXdu3d").unwrap();  // known polkadot node example "12D3KooWDxhWkQ1LYMPcwUpcb7yy272DrMvGUoXH4wjkgzDXdu3d"

    let peer = match lookup.dht_query(peer_query).await {
        Ok(peer) => peer,
        Err(e) => panic!("{:?}",e)
        
    };
    println!("Found {:?} {:?} {:?}", peer.peer_id, peer.listen_addrs, peer.protocols);
    lookup.dial(&peer).await;

    println!("Ending Session.");

}

