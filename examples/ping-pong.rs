#![feature(proc_macro, conservative_impl_trait, generators)]
extern crate futures_await as futures;
use futures::stream::Stream;


extern crate unbase;
use unbase::{Network,SubjectHandle};
use std::{thread,time};

/// This example is a rudimentary interaction between two remote nodes
/// As of the time of this writing, the desired convergence properties of the system are not really implemented.
/// For now we are relying on the size of the cluster being smaller than the memo peering target,
/// rather than gossip (once the record has been made resident) or index convergence (prior to the record being located). 
fn main() {

    let t1 = thread::spawn(move || {

        let net1 = Network::create_new_system();
        let udp1 = unbase::network::transport::TransportUDP::new( "127.0.0.1:12001".to_string() );
        net1.add_transport( Box::new(udp1) );
        let context_a = unbase::Slab::new(&net1).create_context();

        // HACK - need to wait until peering of the root index node is established
        // because we are aren't updating the net's root seed, which is what is being sent when peering is established
        // TODO: establish some kind of positive pressure to push out index nodes
        thread::sleep( time::Duration::from_millis(700) );

        println!("A - Sending Initial Ping");
        let rec_a1 = SubjectHandle::new_kv(&context_a, "action", "Ping").unwrap();

        let mut pings = 0;
        for _ in rec_a1.observe().wait() {
            println!("A - VAL {:?}, {}", rec_a1.head_memo_ids(), rec_a1.get_value("action").unwrap());

            if "Pong" == rec_a1.get_value("action").unwrap() {
                println!("A - [ Ping ->       ]");
                rec_a1.set_value("action","Ping").unwrap();
                pings += 1;

                if pings >= 10 {
                    break
                }
            }
        }

    });

    // Ensure slab_a is listening
    thread::sleep( time::Duration::from_millis(50) );

    let t2 = thread::spawn(move || {

        let net2 = unbase::Network::new();
        net2.hack_set_next_slab_id(200);

        let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:12002".to_string());
        net2.add_transport( Box::new(udp2.clone()) );

        let context_b = unbase::Slab::new(&net2).create_context();

        udp2.seed_address_from_string( "127.0.0.1:12001".to_string() );

        println!("B - Waiting for root index seed...");
        context_b.root_index_wait( 1000 ).unwrap();

        println!("B - Searching for Ping record...");
        let rec_b1 = context_b.fetch_kv_wait( "action", "Ping", 1000 ).unwrap(); 
        println!("B - Found Ping record.");

        let mut pongs = 0;
        for _ in rec_b1.observe().wait() {

            if "Ping" == rec_b1.get_value("action").unwrap() {
                println!("B - [       <- Pong ]");
                rec_b1.set_value("action","Pong").unwrap();
                pongs += 1;

                if pongs >= 10 {
                    break
                }
            }

        }

    });
    
    t2.join().expect("thread 2"); // Thread 2 is more likely to panic
    t1.join().expect("thread 1");
}
