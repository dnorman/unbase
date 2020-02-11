use futures::join;
use std::time::Duration;
use timer::Delay;
use tracing::debug;

#[unbase_test_util::async_test]
async fn init_blackhole() {
    unbase_test_util::init_test_logger();

    let net = unbase::Network::create_new_system();
    let blackhole = unbase::network::transport::Blackhole::new();
    net.add_transport(Box::new(blackhole));
}

#[unbase_test_util::async_test]
async fn init_blackhole_slab() {
    unbase_test_util::init_test_logger();

    let net = unbase::Network::create_new_system();
    let blackhole = unbase::network::transport::Blackhole::new();
    net.add_transport(Box::new(blackhole));

    {
        let slab_a = unbase::Slab::initialize(&net);
        let _context_a = slab_a.create_context();
    }

    // Slabs should have been dropped by now
    assert_eq!(net.get_all_local_slabs().len(), 0);
}

#[unbase_test_util::async_test]
async fn init_local_single() {
    unbase_test_util::init_test_logger();

    let net = unbase::Network::create_new_system();
    {
        let slab_a = unbase::Slab::initialize(&net);
        let _context_a = slab_a.create_context();
    }

    // Slabs should have been dropped by now
    assert_eq!(net.get_all_local_slabs().len(), 0);
}

#[unbase_test_util::async_test]
async fn init_local_multi() {
    unbase_test_util::init_test_logger();

    let net = unbase::Network::create_new_system();
    {
        let slab_a = unbase::Slab::initialize(&net);
        let slab_b = unbase::Slab::initialize(&net);
        let slab_c = unbase::Slab::initialize(&net);

        assert!(slab_a.id == 0, "Slab A ID shoud be 0");
        assert!(slab_b.id == 1, "Slab B ID shoud be 1");
        assert!(slab_c.id == 2, "Slab C ID shoud be 2");

        assert!(slab_a.peer_slab_count() == 2, "Slab A Should know two peers");
        assert!(slab_b.peer_slab_count() == 2, "Slab B Should know two peers");
        assert!(slab_c.peer_slab_count() == 2, "Slab C Should know two peers");

        let _context_a = slab_a.create_context();
        Delay::new(Duration::from_millis(50)).await;

        let _context_b = slab_b.create_context();
        let _context_c = slab_c.create_context();
    }

    // TODO: Sometimes not all slabs clean up immediately. This is almost certainly indicative of some
    // kind of bug. There appears to be some occasional laggard thread which is causing a race condition
    // of some kind, and occasionally preventing one of the Slabs from destroying in time. All I know at
    // this point is that adding the delay here seems to help, which implies that it's not a deadlock.
    // Delay::new(Duration::from_millis(5000)).await;

    // We should have zero slabs resident at this point
    //    assert!( net.get_all_local_slabs().len() == 0, "not all slabs have cleaned up" );
}

#[unbase_test_util::async_test]
async fn init_udp() {
    unbase_test_util::init_test_logger();

    let f1 = udp_station_one();
    let f2 = udp_station_two();

    join! {f1, f2};
}

async fn udp_station_one() {
    let net1 = unbase::Network::create_new_system();
    {
        let udp1 = unbase::network::transport::TransportUDP::new("127.0.0.1:12345".to_string());
        net1.add_transport(Box::new(udp1.clone()));
        let slab_a = unbase::Slab::initialize(&net1);

        // TODO - replace these sleeps with timed-out checkpoints of some kind
        Delay::new(Duration::from_millis(150)).await;
        assert_eq!(slab_a.peer_slab_count(), 1);
    }

    // my local slab should have dropped
    assert_eq!(net1.get_all_local_slabs().len(), 0);
}

async fn udp_station_two() {
    let net2 = unbase::Network::new();
    Delay::new(Duration::from_millis(50)).await;
    {
        let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:1337".to_string());
        net2.add_transport(Box::new(udp2.clone()));
        let slab_b = unbase::Slab::initialize(&net2);

        udp2.seed_address_from_string("127.0.0.1:12345".to_string());
        Delay::new(Duration::from_millis(50)).await;

        assert_eq!(slab_b.peer_slab_count(), 1);
    }

    assert_eq!(net2.get_all_local_slabs().len(), 0);
}

#[unbase_test_util::async_test]
async fn avoid_unnecessary_chatter() {
    unbase_test_util::init_test_logger();

    let net = unbase::Network::create_new_system();
    {
        let slab_a = unbase::Slab::initialize(&net);
        let slab_b = unbase::Slab::initialize(&net);

        let _context_a = slab_a.create_context();
        let _context_b = slab_b.create_context();

        Delay::new(Duration::from_millis(100)).await;

        debug!("Slab A count of MemoRefs present {}", slab_a.count_of_memorefs_resident());
        debug!("Slab A count of MemoRefs present {}", slab_b.count_of_memorefs_resident());
        debug!("Slab A count of Memos received {}", slab_a.count_of_memos_received());
        debug!("Slab B count of Memos received {}", slab_a.count_of_memos_received());
        debug!("Slab A count of Memos redundantly received {}",
               slab_a.count_of_memos_reduntantly_received());
        debug!("Slab B count of Memos redundantly received {}",
               slab_a.count_of_memos_reduntantly_received());

        assert!(slab_a.count_of_memos_reduntantly_received() == 0, "Redundant memos received");
        assert!(slab_b.count_of_memos_reduntantly_received() == 0, "Redundant memos received");
    }

    assert!(net.get_all_local_slabs().len() == 0);
}
