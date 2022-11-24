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

## Protocol Test
Build the library, main binary and examples on both nodes A and B:

`cargo build --examples --release`
Run the Responder on A:

`$./target/release/examples/responder`

Wait for the [peer id] and [address] confirmation. If you are not sure about the NAT traversal of this address, the fastest try would be to look for a local address alternative which would be visible between both peers. From within the same host, 127.0.0.1 should work out of the box.

Run the requester with the [peer id] and [address] provided by A, on the other node B:

`$./target/release/examples/requester [peerid] [address]`

usage example:

`$./target/release/examples/requester 12D3KooWDEDDufC8NDPXaUoYrhBkGFWSer9ijkfgZpnHpZXTQPF2 /ip4/127.0.0.1/tcp/34431`

Thank you and enjoy!
;) <3



[1] https://docs.libp2p.io/concepts/nat/