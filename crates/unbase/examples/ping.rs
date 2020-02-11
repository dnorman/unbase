extern crate unbase;
use futures::StreamExt;
use std::time::Duration;
use timer::Delay;
use unbase::{
    network::transport::TransportUDP,
    Entity,
    Network,
    Slab,
};

#[async_std::main]
async fn main() {
    let net = Network::create_new_system();
    let udp = TransportUDP::new("127.0.0.1:12001".to_string());
    net.add_transport(Box::new(udp));
    let slab = Slab::initialize(&net);
    let context_a = slab.create_context();

    println!("A - Started up. You have 7 seconds to start the pong client (yeah this is lame. working on it!)");

    // HACK - need to wait until peering of the root index node is established
    // because we are aren't updating the net's root seed, which is what is being sent when peering is established
    // TODO: establish some kind of positive pressure to push out index nodes
    Delay::new(Duration::from_millis(7000)).await;

    let mut rec_a1 = Entity::new_with_single_kv(&context_a, "action", "Ping").await.unwrap();

    let mut pings = 0;

    while let Some(_) = rec_a1.observe().next().await {
        // HACK - Presently we are relying on the newly issued index leaf for record consistency, which is applied
        // immediately after this event is sent
        Delay::new(Duration::from_millis(10)).await;

        if "Pong" == rec_a1.get_value("action").await.expect("retrieval").expect("has value") {
            println!("A - [ Ping ->       ]");
            rec_a1.set_value("action", "Ping").await.unwrap();
            pings += 1;

            if pings >= 10 {
                break;
            }
        }
    }
}
