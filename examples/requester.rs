use rust_libp2p_kad_swarm as synack_node;
use libp2p::{
    PeerId,
    Multiaddr
};
use std::str::FromStr;

#[async_std::main]
async fn main() {
    let mut a = synack_node::LookupClient::new(
        &synack_node::Network::Kusama
    );
    let expected_peer_id: PeerId = PeerId::from_str("12D3KooWEYuMN7eZHV8bCvZaNE7zXt4E8kjYbvXNxk3m97hhvuyD").unwrap();  // predicted :"12D3KooWEChVMMMzV8acJ53mJHrw1pQ27UAGkCxWXLJutbeUMvVu"
    let expected_address : Multiaddr = Multiaddr::from_str("/ip4/181.43.255.231/tcp/34421").unwrap();
    let _ = a.add_address(expected_peer_id, expected_address).await;

    let _ = a.listen().await;
    let _ = a.send_request(expected_peer_id).await;
    a.init().await;
    // TODO: init the event loop for the protocol
}