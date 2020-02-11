extern crate unbase;

use futures::{
    join,
    StreamExt,
};
use std::time::Duration;
use timer::Delay;
use unbase::{
    network::transport::TransportUDP,
    Entity,
    Network,
    Slab,
};

/// This example is a rudimentary interaction between two remote nodes
/// As of the time of this writing, the desired convergence properties of the system are not really implemented.
/// For now we are relying on the size of the cluster being smaller than the memo peering target,
/// rather than gossip (once the record has been made resident) or index convergence (prior to the record being
/// located).
#[async_std::main]
async fn main() {
    unbase_test_util::init_test_logger();

    let p1 = player_one();
    let p2 = player_two();

    join! { p1, p2 };
}

async fn player_one() {
    let net1 = Network::create_new_system();
    let udp1 = TransportUDP::new("127.0.0.1:12001".to_string());
    net1.add_transport(Box::new(udp1));
    let slab = Slab::initialize(&net1);
    let context_a = slab.create_context();

    // HACK - need to wait until peering of the root index node is established
    // because we are aren't updating the net's root seed, which is what is being sent when peering is established
    // TODO: establish some kind of positive pressure to push out index nodes
    Delay::new(Duration::from_millis(700)).await;

    println!("A - Sending Initial Ping");
    let mut rec_a1 = Entity::new_with_single_kv(&context_a, "action", "Ping").await.unwrap();

    let mut pings = 0;

    while let Some(_head) = rec_a1.observe().next().await {
        // HACK - Presently we are relying on the newly issued index leaf for record consistency, which is applied
        // immediately after this event is sent
        Delay::new(Duration::from_millis(10)).await;

        let value = rec_a1.get_value("action").await.expect("retrieval").expect("found");

        if "Pong" == value {
            println!("A - [ Ping ->       ]");
            rec_a1.set_value("action", "Ping").await.unwrap();
            pings += 1;

            if pings >= 10 {
                break;
            }
        }
    }
}

async fn player_two() {
    let net2 = Network::new();

    let udp2 = TransportUDP::new("127.0.0.1:12002".to_string());
    net2.add_transport(Box::new(udp2.clone()));

    let slab = Slab::initialize(&net2);
    let context_b = slab.create_context();

    udp2.seed_address_from_string("127.0.0.1:12001".to_string());

    println!("B - Waiting for root index seed...");
    context_b.root_index().await.unwrap();

    println!("B - Searching for Ping record...");
    let mut rec_b1 = context_b.fetch_kv("action", "Ping", Duration::from_secs(1)).await.unwrap();
    println!("B - Found Ping record.");

    let mut pongs = 0;
    let mut obs = rec_b1.observe();
    while let Some(_) = obs.next().await {
        // HACK - Presently we are relying on the newly issued index leaf for record consistency, which is applied
        // immediately after this event is sent
        Delay::new(Duration::from_millis(10)).await;

        let value = rec_b1.get_value("action").await.expect("it worked").expect("found");

        if "Ping" == value {
            println!("B - [       <- Pong ]");
            rec_b1.set_value("action", "Pong").await.unwrap();
            pongs += 1;

            if pongs >= 10 {
                break;
            }
        }
    }
}
