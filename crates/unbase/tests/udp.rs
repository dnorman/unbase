extern crate unbase;

use std::{thread, time};

#[test]
fn test_udp() {

    let t1 = thread::spawn(|| {

        let net1 = unbase::Network::create_new_system();
        let udp1 = unbase::network::transport::TransportUDP::new("127.0.0.1:12345".to_string());
        net1.add_transport( Box::new(udp1.clone()) );
        let _slab_a = unbase::Slab::new(&net1);

    //    thread::sleep( time::Duration::from_secs(5) );
    });

    thread::sleep( time::Duration::from_millis(50) );

    let t2 = thread::spawn(|| {
        let net2 = unbase::Network::new();
        net2.hack_set_next_slab_id(200);

        let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:1337".to_string());
        net2.add_transport( Box::new(udp2.clone()) );

        let _slab_b = unbase::Slab::new(&net2);

        udp2.seed_address_from_string( "127.0.0.1:12345".to_string() );
        thread::sleep( time::Duration::from_millis(500) );
    });

    t1.join().expect("thread1.join");
    t2.join().expect("thread2.join");
}


#[test]
fn test_udp2() {
    use std::{thread,time};
    use unbase::{Network,Slab,SubjectHandle};
    use unbase::network::transport::TransportUDP;

    thread::spawn(|| {
        let net1 = Network::create_new_system();
        let udp1 = TransportUDP::new("127.0.0.1:12021".to_string());
        net1.add_transport( Box::new(udp1) );
        let context_a = Slab::new(&net1).create_context();
    
        // HACK - wait for slab_b to be on the peer list, and to be hooked in to our root_index_seed
        thread::sleep( time::Duration::from_millis(150) );
        let beast_a = SubjectHandle::new_kv(&context_a, "beast", "Lion").expect("write successful");
        beast_a.set_value("sound","Grraaawrrr").expect("write successful");

        // Hang out so we can help thread 2
        thread::sleep(time::Duration::from_millis(500));
    });
    
    // HACK - Ensure slab_a is listening
    thread::sleep( time::Duration::from_millis(20) );
    
    thread::spawn(|| {
        let net2 = Network::new();
        net2.hack_set_next_slab_id(200);
        let udp2 = TransportUDP::new("127.0.0.1:12022".to_string());
        net2.add_transport( Box::new(udp2.clone()) );
        let slab_b = Slab::new(&net2);
        udp2.seed_address_from_string( "127.0.0.1:12021".to_string() );
        let context_b = slab_b.create_context();
        let beast_b = context_b.fetch_kv_wait("beast","Lion",1000).expect("it worked");
        println!("The {} goes {}", beast_b.get_value("beast").expect("it worked"), beast_b.get_value("sound").expect("it worked") )
    });
}