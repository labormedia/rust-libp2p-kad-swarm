# rust-libp2p-kad-swarm
Rust implementation of a minimal handshake session with Kademlia implementation under rust-libp2p.

While a responder "A" listens, the requester "B" sends a SYN<->SYN message to "A" using its [peer_id] and [address], "A" responds with SYN<->SYNACK and disconnects while "B" expects the SYN<->SYNACK and disconnects which makes this simple handshake conclude under a private, ephemeral and permissionless network connection provided by the rust-libp2p library.

It implements libp2p's kademlia dht networking routing, identify, noise, yamux, relay, ping, keep_alive and request_respond behaviour layers, which are available for node discovery and nat traversal tooling. It also includes two examples as demo, a responder/target and a requester/guest with ephemeral (random) peer ids for p2p connection per execution.

Because of the possible complications, the examples does not consider NAT traversal[1]. It should be able to reach the node's network address both inbound and outbound for it to succeed. On local network this should be trivial.


## Build
`cargo build`

## Run
`cargo run`

## Tests
`cargo test -- --nocapture`

## Protocol Test
Build the library, main binary and examples for both nodes A (responder) and B (requester):

```cargo build --examples --release```

Run the Responder A:

```$./target/release/examples/responder```

Wait for the [peer id] and [address] confirmation. If you are not sure about the NAT traversal of this address, the fastest try would be to look for a local address alternative which would be visible between both peers. From within the same host, 127.0.0.1 should work on most cases.

Run the requester "B" along with the arguments for [peer id] and [address] provided by "A":

```$./target/release/examples/requester [peerid] [address]```

Usage example:

```$./target/release/examples/responder
Local PeerID : PeerId("12D3KooWFFYGHLUYL68rGRyQhYcJTWbLokAJ3c48LGFt5PmG3qeW")
PeerId("12D3KooWDgtynm4S9M3m6ZZhXYu2RrWKdvkCSScc25xKDVSg1Sjd") added in the Routing Table.
PeerId("12D3KooWNpGriWPmf621Lza9UWU9eLLBdCFaErf6d4HSK7Bcqnv4") added in the Routing Table.
PeerId("12D3KooWLmLiB4AenmN2g2mHbhNXbUcNiGi99sAkSk1kAQedp8uE") added in the Routing Table.
PeerId("12D3KooWEGHw84b4hfvXEfyq4XWEmWCbRGuHMHQMpby4BAtZ4xJf") added in the Routing Table.
PeerId("12D3KooWF9KDPRMN8WpeyXhEeURZGP8Dmo7go1tDqi7hTYpxV9uW") added in the Routing Table.
...
Observed peer_id and addresses : PeerId("12D3KooWFFYGHLUYL68rGRyQhYcJTWbLokAJ3c48LGFt5PmG3qeW") "/ip4/127.0.0.1/tcp/43263"
Request received from : PeerId("12D3KooWP9G85K4b6wPeuYqGg6RnQn8217d3KMJBNitmTfWFS2HE") [83, 89, 78]
Response sent to : PeerId("12D3KooWP9G85K4b6wPeuYqGg6RnQn8217d3KMJBNitmTfWFS2HE")
Closing connection.
$ 
```

```$ ./target/release/examples/requester "12D3KooWFFYGHLUYL68rGRyQhYcJTWbLokAJ3c48LGFt5PmG3qeW" /ip4/127.0.0.1/tcp/43263
Arguments: ["./target/release/examples/requester", "12D3KooWFFYGHLUYL68rGRyQhYcJTWbLokAJ3c48LGFt5PmG3qeW", "/ip4/192.168.100.55/tcp/43263"]
Local PeerID : PeerId("12D3KooWP9G85K4b6wPeuYqGg6RnQn8217d3KMJBNitmTfWFS2HE")
New Listen Address : "/ip4/127.0.0.1/tcp/43231"
New Listen Address : "/ip4/192.168.100.55/tcp/43231"
New Listen Address : "/ip4/172.17.0.1/tcp/43231"
Response received : PeerId("12D3KooWFFYGHLUYL68rGRyQhYcJTWbLokAJ3c48LGFt5PmG3qeW") RequestId(1) "SYNACK"
Closing handshake.
$```

Thank you and enjoy!
;) <3



[1] https://docs.libp2p.io/concepts/nat/