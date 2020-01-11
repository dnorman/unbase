extern crate unbase;
use unbase::SubjectHandle;
use std::{thread, time};

#[test]
fn remote_traversal_simulated() {

    let net = unbase::Network::create_new_system();
    let mut simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    simulator.metronome(50);

    let context_a = unbase::Slab::new(&net).create_context();
    let _context_b = unbase::Slab::new(&net).create_context();

    let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo").unwrap();

    rec_a1.set_value("animal_sound","Woof").unwrap();
    rec_a1.set_value("animal_sound","Meow").unwrap();

    // Tick - Now it should have propagated to slab B
    // Tick - now slab A should know that Slab B has it
    simulator.wait_ticks(3);

    context_a.slab.remotize_memo_ids( &rec_a1.get_all_memo_ids() ).expect("failed to remotize memos");

    simulator.wait_ticks(1);

    assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Meow");

}

#[test]
fn remote_traversal_nondeterministic_direct() {

    let net = unbase::Network::create_new_system();
    // Automatically uses LocalDirect, which should be much faster than the simulator, but is also nondeterministic.
    // This will be used in production for slabs that cohabitate the same process

    let context_a  = unbase::Slab::new(&net).create_context();
    let _context_b  = unbase::Slab::new(&net).create_context();

    let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo").unwrap();

    rec_a1.set_value("animal_sound","Woof").unwrap();
    rec_a1.set_value("animal_sound","Meow").unwrap();

    thread::sleep(time::Duration::from_millis(50));

    context_a.slab.remotize_memo_ids_wait( &rec_a1.get_all_memo_ids(), 1000 ).expect("failed to remotize memos");

    thread::sleep(time::Duration::from_millis(50));

    assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Meow");

}

// Playing silly games with timing here in order to make it to work Initially
// Should be make substantially more robust.

// TODO: Remove the sleeps and ensure it still does the right thing ;)
//     this will entail several essential changes like reevaluating
//     memo peering when new SlabPresence is received, etc
#[test]
fn remote_traversal_nondeterministic_udp() {

    let t1 = thread::spawn(|| {

        let net1 = unbase::Network::create_new_system();

        let udp1 = unbase::network::transport::TransportUDP::new("127.0.0.1:12011".to_string());
        net1.add_transport( Box::new(udp1) );

        // no reason to wait to create the context here
        let context_a = unbase::Slab::new(&net1).create_context();

        // HACK - wait for slab_b to be on the peer list, and to be hooked in to our root_index_seed
        thread::sleep( time::Duration::from_millis(150) );

        // Do some stuff
        let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo").unwrap();
        rec_a1.set_value("animal_sound","Woof").unwrap();
        rec_a1.set_value("animal_sound","Meow").unwrap();
        
        context_a.slab.remotize_memo_ids_wait( &rec_a1.get_all_memo_ids(), 1000 ).expect("remotize memos");

        // Not really any strong reason to wait here, except just to play nice and make sure slab_b's peering is updated
        // TODO: test memo expungement/de-peering, followed immediately by MemoRequest for same
        thread::sleep(time::Duration::from_millis(50));

        // now lets see if we can project rec_a1 animal_sound. This will require memo retrieval from slab_b
        assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Meow");


        thread::sleep(time::Duration::from_millis(500));


    });

    // HACK - Ensure slab_a is listening
    thread::sleep( time::Duration::from_millis(50) );

    let t2 = thread::spawn(|| {
        let net2 = unbase::Network::new();
        net2.hack_set_next_slab_id(200);

        let udp2 = unbase::network::transport::TransportUDP::new("127.0.0.1:12012".to_string());
        net2.add_transport( Box::new(udp2.clone()) );

        let slab_b = unbase::Slab::new(&net2);
        udp2.seed_address_from_string( "127.0.0.1:12011".to_string() );

        thread::sleep( time::Duration::from_millis(50) );
        let _context_b = slab_b.create_context();
        // hang out to keep stuff in scope, and hold off calling the destructors
        // necessary in order to be online so we can answer slab_a's inquiries
        thread::sleep(time::Duration::from_millis(1500));


    });

    t2.join().expect("thread2.join");
    t1.join().expect("thread1.join");

}