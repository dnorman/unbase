extern crate unbase;
use futures::join;
use futures_await_test::async_test;
use std::time::Duration;
use timer::Delay;
use tracing::{
    debug,
    info,
    span,
    Level,
};
use unbase::Entity;

#[async_test]
async fn remote_traversal_simulated() {
    unbase_test_util::init_test_logger();

    let testspan = span!(Level::INFO, "remote_traversal_simulated");
    let _enter = testspan.enter();

    let net = unbase::Network::create_new_system();
    let simulator = unbase::util::simulator::Simulator::new();
    net.add_transport(Box::new(simulator.clone()));
    let started = simulator.start();
    debug!(%started);

    let slab_a = unbase::Slab::initialize(&net);
    let slab_b = unbase::Slab::initialize(&net);

    let context_a = slab_a.create_context();
    let _context_b = slab_b.create_context();

    let mut rec_a1 = Entity::new_with_single_kv(&context_a, "animal_sound", "Moo").await.unwrap();

    rec_a1.set_value("animal_sound", "Woof").await.unwrap();

    rec_a1.set_value("animal_sound", "Meow").await.unwrap();

    simulator.quiesce().await;

    let memo_ids = rec_a1.get_all_memo_ids().await.unwrap();
    slab_a.remotize_memos(&memo_ids, Duration::from_secs(1))
          .await
          .expect("failed to remotize memos");

    let value = rec_a1.get_value("animal_sound").await;
    assert_eq!(value, Ok(Some("Meow".to_string())));

    simulator.quiesce_and_stop().await;

    assert_eq!(simulator.get_sent().unwrap(), 37);
    assert_eq!(simulator.get_delivered().unwrap(), 37);
    assert_eq!(simulator.get_clock().unwrap(), 7);
}

#[async_test]
async fn remote_traversal_nondeterministic() {
    unbase_test_util::init_test_logger();

    let net = unbase::Network::create_new_system();
    // Automatically uses LocalDirect, which should be much faster than the simulator, but is also nondeterministic.
    // This will be used in production for slabs that cohabitate the same process

    let slab_a = unbase::Slab::initialize(&net);
    let slab_b = unbase::Slab::initialize(&net);

    let context_a = slab_a.create_context();
    let _context_b = slab_b.create_context();

    let mut rec_a1 = Entity::new_with_single_kv(&context_a, "animal_sound", "Moo").await.unwrap();

    rec_a1.set_value("animal_sound", "Woof").await.unwrap();
    rec_a1.set_value("animal_sound", "Meow").await.unwrap();

    // TODO - provide a deterministic way to wait for quiescence when not using the simulator
    Delay::new(Duration::from_millis(50)).await;

    slab_a.remotize_memos(&rec_a1.get_all_memo_ids().await.unwrap(), Duration::from_secs(1))
          .await
          .expect("failed to remotize memos");

    Delay::new(Duration::from_millis(10)).await;

    let value = rec_a1.get_value("animal_sound").await.unwrap().unwrap();
    assert_eq!(value, "Meow");
}

#[async_test]
async fn remote_traversal_nondeterministic_udp() {
    unbase_test_util::init_test_logger();

    let s1 = udp_station_one();
    let s2 = udp_station_two();

    join! { s1, s2 };
}

async fn udp_station_one() {
    let net1 = unbase::Network::create_new_system();

    let udp1 = unbase::network::transport::TransportUDP::new("127.0.0.1:12011".to_string());
    net1.add_transport(Box::new(udp1.clone()));
    let slab_a = unbase::Slab::initialize(&net1);

    // no reason to wait to create the context here
    let context_a = slab_a.create_context();

    // wait for slab_b to be on the peer list, and to be hooked in to our root_index_seed
    Delay::new(Duration::from_millis(150)).await;

    // Do some stuff
    let mut rec_a1 = Entity::new_with_single_kv(&context_a, "animal_sound", "Moo").await.unwrap();
    rec_a1.set_value("animal_sound", "Woof").await.unwrap();
    rec_a1.set_value("animal_sound", "Meow").await.unwrap();

    // TODO - come up with a way to enforce determinism with real network traffic
    Delay::new(Duration::from_millis(50)).await;

    // manually remove the memos
    slab_a.remotize_memos(&rec_a1.get_all_memo_ids().await.unwrap(), Duration::from_secs(1))
          .await
          .expect("failed to remotize memos");

    // Not really any strong reason to wait here, except just to play nice and make sure slab_b's peering is updated
    // TODO: test memo expungement/de-peering, followed immediately by MemoRequest for same
    Delay::new(Duration::from_millis(50)).await;

    // now lets see if we can project rec_a1 animal_sound. This will require memo retrieval from slab_b
    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Meow");

    // can't free the slab yet, because we have to respond to traffic from station two
    Delay::new(Duration::from_millis(500)).await;

    // slab_a drops and goes away

    info!("UDP Station 1 exiting");
}

async fn udp_station_two() {
    let net2 = unbase::Network::new();

    // HACK - Ensure slab_a is listening - TODO make this auto-retry
    Delay::new(Duration::from_millis(50)).await;

    let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:12012".to_string());
    net2.add_transport(Box::new(udp2.clone()));

    let slab_b = unbase::Slab::initialize(&net2);
    udp2.seed_address_from_string("127.0.0.1:12011".to_string());

    Delay::new(Duration::from_millis(50)).await;
    let _context_b = slab_b.create_context();

    // hang out to keep stuff in scope, and hold off calling the destructors
    // necessary in order to be online so we can answer slab_a's inquiries
    Delay::new(Duration::from_millis(800)).await;

    info!("UDP Station 2 exiting");
}
