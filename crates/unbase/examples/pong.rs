use futures::StreamExt;
use unbase::{
    network::transport::TransportUDP,
    Network,
    Slab,
};

use std::time::Duration;
use timer::Delay;

#[async_std::main]
async fn main() {
    let net = Network::new();

    let udp = TransportUDP::new("127.0.0.1:12002".to_string());
    net.add_transport(Box::new(udp.clone()));

    let slab = Slab::initialize(&net);
    let context = slab.create_context();

    println!("B - REMEMBER TO START THE PING EXAMPLE FIRST!");

    udp.seed_address_from_string("127.0.0.1:12001".to_string());

    println!("B - Waiting for root index seed...");
    context.root_index().await.unwrap();

    println!("B - Searching for Ping record...");
    let mut record = context.fetch_kv("action", "Ping", Duration::from_secs(30)).await.unwrap();
    println!("B - Found Ping record.");

    let mut pongs = 0;
    let mut obs = record.observe();
    while let Some(_) = obs.next().await {
        // HACK - Presently we are relying on the newly issued index leaf for record consistency, which is applied
        // immediately after this event is sent
        Delay::new(Duration::from_millis(10)).await;

        let value = record.get_value("action").await.expect("it worked").expect("found");
        if "Ping" == value {
            println!("B - [       <- Pong ]");
            record.set_value("action", "Pong").await.unwrap();
            pongs += 1;

            if pongs >= 10 {
                break;
            }
        }
    }
}
