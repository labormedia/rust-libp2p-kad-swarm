# rust-libp2p-kad-swarm
Rust implementation of a minimal handshake session with Kademlia implementation under rust-libp2p.

It implements libp2p's kademlia dht networking routing, identify, relay, ping, keep_alive and request_respond behaviour layers, which we will be available for node discovery and nat traversal tooling. It includes two examples as demo, one responder/target and a requester/guest with ephemeral (random) peer ids for p2p connection per execution.

Because of the possible complications, the examples does not consider NAT traversal[1], so you should be able to reach the node's network address both inbound and outbound. On local network this should be trivial.


## Build
`cargo build`

## Run
`cargo run`

## Tests
`cargo test -- --nocapture`



[1] https://docs.libp2p.io/concepts/nat/