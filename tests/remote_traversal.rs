extern crate unbase;
use unbase::SubjectHandle;
use std::{thread, time};

#[test]
fn remote_traversal_simulated() {

    let net = unbase::Network::create_new_system();
    let simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let context_a = slab_a.create_context();
    let _context_b = slab_b.create_context();

    let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo").unwrap();

    rec_a1.set_value("animal_sound","Woof");
    rec_a1.set_value("animal_sound","Meow");

    simulator.advance_clock(1); // Now it should have propagated to slab B

    simulator.advance_clock(1); // now slab A should know that Slab B has it

    slab_a.remotize_memo_ids( &rec_a1.get_all_memo_ids() ).expect("failed to remotize memos");

    simulator.advance_clock(1);

    // Thread is necessary to prevent retrieval deadlock, as the simulator is controlled in this thead
    // This should be reconsidered when the simulator is reworked per https://github.com/unbase/unbase/issues/6
    let handle = thread::spawn(move || {

        assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Meow");

    });

    // HACK HACK HACK HACK - clearly we have a deficiency in the simulator / threading model
    let ten_millis = time::Duration::from_millis(10);
    thread::sleep(ten_millis);

    simulator.advance_clock(1);

    simulator.advance_clock(1);

    handle.join().unwrap();

}

#[test]
fn remote_traversal_nondeterministic() {


    let net = unbase::Network::create_new_system();
    // Automatically uses LocalDirect, which should be much faster than the simulator, but is also nondeterministic.
    // This will be used in production for slabs that cohabitate the same process

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let context_a = slab_a.create_context();
    let _context_b = slab_b.create_context();

    let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo").unwrap();

    rec_a1.set_value("animal_sound","Woof");
    rec_a1.set_value("animal_sound","Meow");

    thread::sleep(time::Duration::from_millis(50));

    slab_a.remotize_memo_ids( &rec_a1.get_all_memo_ids() ).expect("failed to remotize memos");

    thread::sleep(time::Duration::from_millis(50));


    let handle = thread::spawn(move || {

        assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Meow");

    });

    thread::sleep(time::Duration::from_millis(50));

    handle.join().unwrap();

}

/// Playing silly games with timing here in order to make it to work Initially
/// Should be make substantially more robust.
///
/// TODO: Remove the sleeps and ensure it still does the right thing ;)
///       this will entail several essential changes like reevaluating
///       memo peering when new SlabPresence is received, etc
#[test]
fn remote_traversal_nondeterministic_udp() {

    let t1 = thread::spawn(|| {

        let net1 = unbase::Network::create_new_system();

        let udp1 = unbase::network::transport::TransportUDP::new("127.0.0.1:12001".to_string());
        net1.add_transport( Box::new(udp1.clone()) );
        let slab_a = unbase::Slab::new(&net1);

        // no reason to wait to create the context here
        let context_a = slab_a.create_context();

        // wait for slab_b to be on the peer list, and to be hooked in to our root_index_seed
        thread::sleep( time::Duration::from_millis(150) );

        // Do some stuff
        let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo").unwrap();
        rec_a1.set_value("animal_sound","Woof");
        rec_a1.set_value("animal_sound","Meow");

        // Wait until it's been replicated
        thread::sleep(time::Duration::from_millis(150));

        // manually remove the memos
        slab_a.remotize_memo_ids( &rec_a1.get_all_memo_ids() ).expect("failed to remotize memos");

        // Not really any strong reason to wait here, except just to play nice and make sure slab_b's peering is updated
        // TODO: test memo expungement/de-peering, followed immediately by MemoRequest for same
        thread::sleep(time::Duration::from_millis(50));

        // now lets see if we can project rec_a1 animal_sound. This will require memo retrieval from slab_b
        assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Meow");


        thread::sleep(time::Duration::from_millis(500));


    });

    // Ensure slab_a is listening
    thread::sleep( time::Duration::from_millis(50) );

    let t2 = thread::spawn(|| {
        let net2 = unbase::Network::new();
        net2.hack_set_next_slab_id(200);

        let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:12002".to_string());
        net2.add_transport( Box::new(udp2.clone()) );

        let slab_b = unbase::Slab::new(&net2);
        udp2.seed_address_from_string( "127.0.0.1:12001".to_string() );

        thread::sleep( time::Duration::from_millis(50) );
        let _context_b = slab_b.create_context();
        // hang out to keep stuff in scope, and hold off calling the destructors
        // necessary in order to be online so we can answer slab_a's inquiries
        thread::sleep(time::Duration::from_millis(1500));


    });

    t2.join().expect("thread2.join");
    t1.join().expect("thread1.join");

}
