#![feature(async_closure)]

extern crate unbase;

use timer::Delay;
use std::time::Duration;
use futures_await_test::async_test;
use futures::{
    future::{
        FutureExt
    },
    join,
};
use tracing::info;

#[async_test]
async fn test_udp() {
    unbase_test_util::init_test_logger();

    let t1 = test1_node_a();
    let t2 = test1_node_b();

    join!{ t1, t2 };
}


async fn test1_node_a() {
    let net1 = unbase::Network::create_new_system();
    let udp1 = unbase::network::transport::TransportUDP::new("127.0.0.1:12345".to_string());
    net1.add_transport(Box::new(udp1.clone()));
    let _slab_a = unbase::Slab::new(&net1);
    Delay::new(Duration::from_millis(500)).await;

    info!("Node A is done!");
}

async fn test1_node_b() {
    Delay::new(Duration::from_millis(50)).await;

    let net2 = unbase::Network::new();
    net2.hack_set_next_slab_id(200);

    let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:1337".to_string());
    net2.add_transport(Box::new(udp2.clone()));

    let _slab_b = unbase::Slab::new(&net2);

    udp2.seed_address_from_string("127.0.0.1:12345".to_string());
    Delay::new(Duration::from_millis(500)).await;

    info!("Node B is done!");
    // TODO improve this test to actually exchange something, or at least verify that we've retrieved the root index
}

#[async_test]
async fn test_udp2() {
    use unbase::{Network,Slab,SubjectHandle};
    use unbase::network::transport::TransportUDP;

    unbase_test_util::init_test_logger();

    let t1 = test2_node_a();
    let t2 = test2_node_b();

    join!{ t1, t2 };
}

async fn test2_node_a() {
    let net1 = Network::create_new_system();
    let udp1 = TransportUDP::new("127.0.0.1:12021".to_string());
    net1.add_transport(Box::new(udp1));
    let context_a = Slab::new(&net1).create_context();

    // HACK - wait for slab_b to be on the peer list, and to be hooked in to our root_index_seed
    Delay::new(Duration::from_millis(150)).await;
    let beast_a = SubjectHandle::new_kv(&context_a, "beast", "Lion").expect("write successful");
    beast_a.set_value("sound", "Grraaawrrr").expect("write successful");

    // Hang out so we can help task 2
    Delay::new(Duration::from_millis(500)).await;
}

async fn test2_node_b() {
    // HACK - Ensure slab_a is listening
    Delay::new(Duration::from_millis(20)).await;

    let net2 = Network::new();
    net2.hack_set_next_slab_id(200);
    let udp2 = TransportUDP::new("127.0.0.1:12022".to_string());
    net2.add_transport(Box::new(udp2.clone()));
    let slab_b = Slab::new(&net2);
    udp2.seed_address_from_string("127.0.0.1:12021".to_string());
    let context_b = slab_b.create_context();
    let beast_b = context_b.fetch_kv_wait("beast", "Lion", 1000).expect("it worked");
    println!("The {} goes {}", beast_b.get_value("beast").expect("it worked"), beast_b.get_value("sound").expect("it worked"))
    ;
}