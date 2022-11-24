use rust_libp2p_kad_swarm::*;

fn main() {
    println!("Hello Requester")
}

fn ping_protocol() {
    let ping = Ping("ping".to_string().into_bytes());
    let pong = Pong("pong".to_string().into_bytes());

    let protocols = iter::once((PingProtocol(), ProtocolSupport::Full));
    let cfg = RequestResponseConfig::default();

    let (peer1_id, trans) = mk_transport();
    let ping_proto1 = RequestResponse::new(PingCodec(), protocols.clone(), cfg.clone());
    let mut swarm1 = Swarm::new(trans, ping_proto1, peer1_id);

    let (peer2_id, trans) = mk_transport();
    let ping_proto2 = RequestResponse::new(PingCodec(), protocols, cfg);
    let mut swarm2 = Swarm::new(trans, ping_proto2, peer2_id);

    let (mut tx, mut rx) = mpsc::channel::<Multiaddr>(1);

    let addr = "/ip4/127.0.0.1/tcp/0".parse().unwrap();
    swarm1.listen_on(addr).unwrap();

    let expected_ping = ping.clone();
    let expected_pong = pong.clone();

    let peer1 = async move {
        loop {
            match swarm1.select_next_some().await {
                SwarmEvent::NewListenAddr { address, .. } => tx.send(address).await.unwrap(),
                SwarmEvent::Behaviour(RequestResponseEvent::Message {
                    peer,
                    message:
                        RequestResponseMessage::Request {
                            request, channel, ..
                        },
                }) => {
                    assert_eq!(&request, &expected_ping);
                    assert_eq!(&peer, &peer2_id);
                    swarm1
                        .behaviour_mut()
                        .send_response(channel, pong.clone())
                        .unwrap();
                }
                SwarmEvent::Behaviour(RequestResponseEvent::ResponseSent { peer, .. }) => {
                    assert_eq!(&peer, &peer2_id);
                }
                SwarmEvent::Behaviour(e) => panic!("Peer1: Unexpected event: {:?}", e),
                _ => {}
            }
        }
    };

    let num_pings: u8 = rand::thread_rng().gen_range(1..100);

    let peer2 = async move {
        let mut count = 0;
        let addr = rx.next().await.unwrap();
        swarm2.behaviour_mut().add_address(&peer1_id, addr.clone());
        let mut req_id = swarm2.behaviour_mut().send_request(&peer1_id, ping.clone());
        assert!(swarm2.behaviour().is_pending_outbound(&peer1_id, &req_id));

        loop {
            match swarm2.select_next_some().await {
                SwarmEvent::Behaviour(RequestResponseEvent::Message {
                    peer,
                    message:
                        RequestResponseMessage::Response {
                            request_id,
                            response,
                        },
                }) => {
                    count += 1;
                    assert_eq!(&response, &expected_pong);
                    assert_eq!(&peer, &peer1_id);
                    assert_eq!(req_id, request_id);
                    if count >= num_pings {
                        return;
                    } else {
                        req_id = swarm2.behaviour_mut().send_request(&peer1_id, ping.clone());
                    }
                }
                SwarmEvent::Behaviour(e) => panic!("Peer2: Unexpected event: {:?}", e),
                _ => {}
            }
        }
    };

    async_std::task::spawn(Box::pin(peer1));
    let () = async_std::task::block_on(peer2);
}

fn mk_transport() -> (PeerId, transport::Boxed<(PeerId, StreamMuxerBox)>) {
    let id_keys = identity::Keypair::generate_ed25519();
    let peer_id = id_keys.public().to_peer_id();

    (
        peer_id,
        tcp::async_io::Transport::new(tcp::Config::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(NoiseAuthenticated::xx(&id_keys).unwrap())
            .multiplex(libp2p::yamux::YamuxConfig::default())
            .boxed(),
    )
}