// Example usage for dialing a peer

use rust_libp2p_kad_swarm as synack_node;
use libp2p::core::{
    PeerId,
    Multiaddr
};
use std::str::FromStr;
use test_protocol;

#[async_std::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    println!("Arguments: {:?}", args);
    let mut a = synack_node::LookupClient::new(
        &synack_node::Network::Kusama
    );
    if &args.len() < &3 {
        usage_message(); 
        panic!("Expected parameters")
    }
    let expected_peer_id: PeerId = match PeerId::from_str(&args[1]) {
        Ok(peer) => peer,
        Err(e) => {
            usage_message();
            panic!("{e:}")
        }
    };  // special case :"12D3KooWEChVMMMzV8acJ53mJHrw1pQ27UAGkCxWXLJutbeUMvVu"
    let expected_address = match Multiaddr::from_str(&args[2]) {
        Ok(address) => address,
        Err(e) => {
            usage_message();
            panic!("{e:}")
        }
    };
    let _ = a.add_address(expected_peer_id, expected_address).await;

    let _ = a.listen().await;
    let payload = test_protocol::SYN("SYN".to_string().into_bytes());
    let _ = a.send_request(expected_peer_id, payload).await;
    match a.init_protocol().await {
        Ok(peer) => {
            println!("Handshake with {:?} succeded.", peer);
        }
        Err(e) => panic!("There was an error : {:?}",e)
    }
}

fn usage_message() {
    println!("
    Usage: ./target/debug/examples/requester [peer_id] [multiaddress]
    ")
}