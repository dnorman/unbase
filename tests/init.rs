//extern crate unbase;
//
//use std::{thread, time};
//
//
//#[test]
//fn init_blackhole() {
//    let net = unbase::Network::create_new_system();
//    let blackhole = unbase::network::transport::Blackhole::new();
//    net.add_transport( Box::new(blackhole) );
//    {
//        let slab_a = unbase::Slab::new(&net);
//        let _context_a = slab_a.create_context();
//    }
//
//    // Slabs should have been dropped by now
//    assert_eq!( net.get_all_local_slabs().len(), 0 );
//}
//
//#[test]
//fn init_local_single() {
//    let net = unbase::Network::create_new_system();
//    {
//        let slab_a = unbase::Slab::new(&net);
//        let _context_a = slab_a.create_context();
//    }
//
//    // Slabs should have been dropped by now
//    assert_eq!( net.get_all_local_slabs().len(), 0 );
//}
//
//#[test]
//fn init_local_multi() {
//
//    let net = unbase::Network::create_new_system();
//    {
//        let slab_a = unbase::Slab::new(&net);
//        let slab_b = unbase::Slab::new(&net);
//        let slab_c = unbase::Slab::new(&net);
//
//        assert!(slab_a.id == 0, "Slab A ID shoud be 0");
//        assert!(slab_b.id == 1, "Slab B ID shoud be 1");
//        assert!(slab_c.id == 2, "Slab C ID shoud be 2");
//
//
//        assert!(slab_a.peer_slab_count() == 2, "Slab A Should know two peers" );
//        assert!(slab_b.peer_slab_count() == 2, "Slab B Should know two peers" );
//        assert!(slab_c.peer_slab_count() == 2, "Slab C Should know two peers" );
//
//        let _context_a = slab_a.create_context();
//        thread::sleep( time::Duration::from_millis(50) );
//
//        let _context_b = slab_b.create_context();
//        let _context_c = slab_c.create_context();
//    }
//
//    // TODO: Sometimes not all slabs clean up immediately. This is almost certainly indicative of some
//    // kind of bug. There appears to be some occasional laggard thread which is causing a race condition
//    // of some kind, and occasionally preventing one of the Slabs from destroying in time. All I know at
//    // this point is that adding the sleep here seems to help, which implies that it's not a deadlock.
//    //thread::sleep( time::Duration::from_millis(5000) );
//
//    // We should have zero slabs resident at this point
//    //assert!( net.get_all_local_slabs().is_empty(), "not all slabs have cleaned up" );
//}
//
//#[test]
//fn init_udp() {
//
//    let t1 = thread::spawn(|| {
//     {
//        let net1 = unbase::Network::create_new_system();
//        {
//            let udp1 = unbase::network::transport::TransportUDP::new("127.0.0.1:12345".to_string());
//            net1.add_transport( Box::new(udp1.clone()) );
//            let slab_a = unbase::Slab::new(&net1);
//            thread::sleep( time::Duration::from_millis(150) );
//            assert_eq!( slab_a.peer_slab_count(), 1 );
//        }
//
//        // my local slab should have dropped
//        assert_eq!( net1.get_all_local_slabs().len(), 0 );
//
//    }
//
//    });
//
//    thread::sleep( time::Duration::from_millis(50) );
//
//    let t2 = thread::spawn(|| {
//        {
//            let net2 = unbase::Network::new();
//            net2.hack_set_next_slab_id(200);
//
//            {
//                let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:1337".to_string());
//                net2.add_transport( Box::new(udp2.clone()) );
//
//                let slab_b = unbase::Slab::new(&net2);
//
//                udp2.seed_address_from_string( "127.0.0.1:12345".to_string() );
//                thread::sleep( time::Duration::from_millis(50) );
//
//                assert_eq!( slab_b.peer_slab_count(), 1 );
//            }
//
//            assert_eq!( net2.get_all_local_slabs().len(), 0 );
//        }
//
//    });
//
//    t1.join().expect("thread1.join");
//    t2.join().expect("thread2.join");
//}
//
//#[test]
//fn avoid_unnecessary_chatter() {
//
//    let net = unbase::Network::create_new_system();
//    {
//        let slab_a = unbase::Slab::new(&net);
//        let slab_b = unbase::Slab::new(&net);
//
//        let _context_a = slab_a.create_context();
//        let _context_b = slab_b.create_context();
//
//        thread::sleep(time::Duration::from_millis(100));
//
//        println!("Slab A count of MemoRefs present {}", slab_a.count_of_memorefs_resident() );
//        println!("Slab A count of MemoRefs present {}", slab_b.count_of_memorefs_resident() );
//
//        println!("Slab A count of Memos received {}", slab_a.count_of_memos_received() );
//        println!("Slab B count of Memos received {}", slab_a.count_of_memos_received() );
//
//        println!("Slab A count of Memos redundantly received {}", slab_a.count_of_memos_reduntantly_received() );
//        println!("Slab B count of Memos redundantly received {}", slab_a.count_of_memos_reduntantly_received() );
//
//        assert!( slab_a.count_of_memos_reduntantly_received() == 0, "Redundant memos received" );
//        assert!( slab_b.count_of_memos_reduntantly_received() == 0, "Redundant memos received" );
//    }
//
//    assert!( net.get_all_local_slabs().is_empty() );
//}
//
//
//#[test]
//fn many_threads() {
//    let net = unbase::Network::new();
//
//    let mut threads = Vec::new();
//    for _ in 0..20 {
//        let net = net.clone();
//
//        threads.push(thread::spawn(move || {
//            let slab = unbase::Slab::new(&net);
//            assert!(slab.id > 0, "Nonzero Slab ID");
//            println!("# info test thread. Slab: {}", slab.id);
//        }));
//    }
//
//    for t in threads {
//        t.join().unwrap();
//    }
//
//    // println!("# {:?}", net);
//}
//
