use futures::join;
use futures_await_test::async_test;
use std::time::Duration;
use timer::Delay;
use unbase::{
    network::transport::TransportUDP,
    Entity,
    Network,
    Slab,
};

use tracing::info;

#[unbase_test_util::async_test]
async fn test_udp1() {
    unbase_test_util::init_test_logger();

    let t1 = test1_node_a();
    let t2 = test1_node_b();

    join! { t1, t2 };
}

async fn test1_node_a() {
    let net = unbase::Network::create_new_system();
    let udp = unbase::network::transport::TransportUDP::new("127.0.0.1:51001".to_string());
    net.add_transport(Box::new(udp.clone()));
    let _slab = unbase::Slab::initialize(&net);

    Delay::new(Duration::from_millis(500)).await;

    info!("Node A is done!");
}

async fn test1_node_b() {
    // HACK - Ensure slab_a is listening
    Delay::new(Duration::from_millis(50)).await;

    let net = unbase::Network::new();
    let udp = unbase::network::transport::TransportUDP::new("127.0.0.1:51002".to_string());
    net.add_transport(Box::new(udp.clone()));
    let _slab = unbase::Slab::initialize(&net);

    udp.seed_address_from_string("127.0.0.1:51001".to_string());
    Delay::new(Duration::from_millis(500)).await;

    info!("Node B is done!");
    // TODO improve this test to actually exchange something, or at least verify that we've retrieved the root index
}

#[async_test]
async fn test_udp2() {
    unbase_test_util::init_test_logger();

    let t1 = test2_node_a();
    let t2 = test2_node_b();

    join! { t1, t2 };
}

async fn test2_node_a() {
    let net = Network::create_new_system();
    let udp = TransportUDP::new("127.0.0.1:52001".to_string());
    net.add_transport(Box::new(udp));

    let slab_a = Slab::initialize(&net);
    let context_a = slab_a.create_context();

    // HACK - wait for slab_b to be on the peer list, and to be hooked in to our root_index_seed
    Delay::new(Duration::from_millis(150)).await;

    let mut beast_a = Entity::new_with_single_kv(&context_a, "beast", "Lion").await
                                                                             .expect("write successful");
    beast_a.set_value("sound", "Grraaawrrr").await.expect("write successful");

    // Hang out so we can help task 2
    Delay::new(Duration::from_millis(500)).await;
}

async fn test2_node_b() {
    // HACK - Ensure slab_a is listening
    Delay::new(Duration::from_millis(50)).await;

    let net2 = Network::new();
    let udp2 = TransportUDP::new("127.0.0.1:52002".to_string());
    net2.add_transport(Box::new(udp2.clone()));
    let slab_b = Slab::initialize(&net2);

    udp2.seed_address_from_string("127.0.0.1:52001".to_string());
    let context_b = slab_b.create_context();

    let mut beast_b = context_b.fetch_kv("beast", "Lion", Duration::from_secs(1))
                               .await
                               .expect("fetch_kv");
    info!("The {} goes {}",
          beast_b.get_value("beast").await.expect("it worked").expect("has value"),
          beast_b.get_value("sound").await.expect("it worked").expect("has value"));
}
