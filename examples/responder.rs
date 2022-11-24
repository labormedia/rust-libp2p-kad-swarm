use rust_libp2p_kad_swarm as synack_node;

#[async_std::main]
async fn main() {
    let mut a = synack_node::LookupClient::from_base64(
        "CAESQL6vdKQuznQosTrW7FWI9At+XX7EBf0BnZLhb6w+N+XSQSdfInl6c7U4NuxXJlhKcRBlBw9d0tj2dfBIVf6mcPA=", 
        &synack_node::Network::Kusama
    );
    let _ = a.listen().await;
    let request_peer_address = a.init().await;
    println!("SYN from node[{:?}]", request_peer_address);
}