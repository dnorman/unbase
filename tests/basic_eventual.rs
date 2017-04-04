extern crate unbase;
use unbase::subject::Subject;
use unbase::error::*;

#[test]
fn basic_sharing() {

    let net = unbase::Network::create_new_system();
    let simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let context_a = slab_a.create_context();
    let context_b = slab_b.create_context();

    let rec_a1 = Subject::new_kv(&context_a, "animal_sound", "Moo").expect("Expected creation to work.");
    assert!(rec_a1.get_value("animal_sound").unwrap() == "Moo", "New subject should be internally consistent");

    simulator.advance_clock(1); // Slab B should have it by now

    let rec_b1 = context_b.get_subject_by_id( rec_a1.id ).expect("get_subject_by_id");

    assert!(rec_b1.get_value("animal_sound").unwrap() == "Moo", "animal sound should be the same as slab a");

    rec_b1.set_value("animal_sound", "cluck");

    assert!(rec_b1.get_value("animal_sound").unwrap() == "cluck", "animal sound should reflect local change");

    let count_a = slab_a.count_of_memos_received();

    simulator.advance_clock(1); // Slab A should have the new value now.

    assert!( slab_a.count_of_memos_received() > count_a );

    assert!(rec_a1.get_value("animal_sound").unwrap() == "cluck", "animal sound should be the same as slab b's new value");
}

#[test]
fn concurrent_update() {

    let net = unbase::Network::create_new_system();
    let simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let context_a = slab_a.create_context();
    let context_b = slab_b.create_context();

    let rec_a1 = Subject::new_kv(&context_a, "animal_sound", "Moo").expect("Expected creation to work.");
    assert!(rec_a1.get_value("animal_sound").unwrap() == "Moo", "New subject should be internally consistent");

    simulator.advance_clock(1); // Slab B should have it by now

    let rec_b1 = context_b.get_subject_by_id( rec_a1.id ).expect("get_subject_by_id");

    assert!(rec_b1.get_value("animal_sound").unwrap() == "Moo", "animal sound should be the same as slab a");

    rec_b1.set_value("animal_sound", "cluck");
    rec_a1.set_value("animal_sound", "woof");

    assert!(rec_a1.get_value("animal_sound").unwrap() == "woof", "a should have its new local value of 'woof'");
    assert!(rec_b1.get_value("animal_sound").unwrap() == "cluck", "b should have its new local value of 'cluck'");


    println!("A: {}, B: {}", rec_a1.get_value("animal_sound").unwrap(),rec_b1.get_value("animal_sound").unwrap());


    simulator.advance_clock(1); // both slabs should exchange memos.

    assert_eq!( rec_a1.get_value("animal_sound").unwrap(), rec_b1.get_value("animal_sound").unwrap() );

}

#[test]
fn basic_eventual() {

    let net = unbase::Network::create_new_system();
    let simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);
    let slab_c = unbase::Slab::new(&net);

    assert!(slab_a.id == 0, "Slab A ID should be 0");
    assert!(slab_b.id == 1, "Slab B ID should be 1");
    assert!(slab_c.id == 2, "Slab C ID should be 2");


    assert!(slab_a.peer_slab_count() == 2, "Slab A Should know two peers" );
    assert!(slab_b.peer_slab_count() == 2, "Slab B Should know two peers" );
    assert!(slab_c.peer_slab_count() == 2, "Slab C Should know two peers" );


    let context_a = slab_a.create_context();
    let context_b = slab_b.create_context();
    let context_c = slab_c.create_context();

    let rec_a1 = Subject::new_kv(&context_a, "animal_sound", "Moo");

    assert!(rec_a1.is_ok(), "New subject should be created");
    let rec_a1 = rec_a1.unwrap();

    assert!(rec_a1.get_value("animal_sound").unwrap() == "Moo", "New subject should be internally consistent");

    // consolidation is necessary for eventual consistency to work
    //context_a.fully_consolidate();

    // These are going to be fairly variable now that we are using memo-based indexing
    // TODO: Find a better way to measure the intent here

    //assert_eq!(slab_a.count_of_memorefs_resident(), 2, "Slab A should have 2 memorefs resident");
    //assert_eq!(slab_b.count_of_memorefs_resident(), 0, "Slab B should have 1 memorefs resident");
    //assert_eq!(slab_c.count_of_memorefs_resident(), 0, "Slab C should have 1 memorefs resident");

    assert!(context_b.get_subject_by_id( rec_a1.id ).unwrap_err() == RetrieveError::NotFound, "new subject should not yet have conveyed to slab B");
    assert!(context_c.get_subject_by_id( rec_a1.id ).unwrap_err() == RetrieveError::NotFound, "new subject should not yet have conveyed to slab C");

    simulator.advance_clock(1); // advance the simulator clock by one tick

    //assert!(slab_a.count_of_memorefs_resident() == 2, "Slab A should have 2 memorefs resident");
    //assert!(slab_b.count_of_memorefs_resident() == 2, "Slab B should have 2 memorefs resident");
    //assert!(slab_c.count_of_memorefs_resident() == 2, "Slab C should have 2 memorefs resident");

    // HERE - in the case of eventual consistency, it might take several
    // seconds for this to convey – not just a single clock tick
    // We've made the index artificially chatty for now, but this will
    // change to a timeout-based process once context::subject_graph is working

    let rec_b1 = context_b.get_subject_by_id( rec_a1.id );
    let rec_c1 = context_c.get_subject_by_id( rec_a1.id );

    assert!(rec_b1.is_ok(), "new subject should now have conveyed to slab B");
    assert!(rec_c1.is_ok(), "new subject should now have conveyed to slab C");

    let rec_b1 = rec_b1.unwrap();
    let rec_c1 = rec_c1.unwrap();

    assert!(rec_b1.get_value("animal_sound").unwrap() == "Moo", "Subject read from Slab B should be internally consistent");
    assert!(rec_c1.get_value("animal_sound").unwrap() == "Moo", "Subject read from Slab C should be internally consistent");

    simulator.advance_clock(1); // advance the simulator clock by one tick

    assert_eq!(rec_a1.get_value("animal_sound").unwrap(), "Moo");
    assert_eq!(rec_b1.get_value("animal_sound").unwrap(), "Moo");
    assert_eq!(rec_c1.get_value("animal_sound").unwrap(), "Moo");


    rec_b1.set_value("animal_type","Bovine");
    assert_eq!(rec_b1.get_value("animal_type").unwrap(), "Bovine");
    assert_eq!(rec_b1.get_value("animal_sound").unwrap(),   "Moo");

    rec_b1.set_value("animal_sound","Woof");
    rec_b1.set_value("animal_type","Kanine");
    assert_eq!(rec_b1.get_value("animal_sound").unwrap(), "Woof");
    assert_eq!(rec_b1.get_value("animal_type").unwrap(),  "Kanine");

    // Should not yet have propagated to slab A
    assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Moo");
    assert!(rec_a1.get_value("animal_type").is_none(), "Should not yet have a value on Slab A for animal_type");

    simulator.advance_clock(1); // advance the simulator clock by one tick

    // Nowwww it should have propagated
    assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Woof");
    assert_eq!(rec_a1.get_value("animal_type").unwrap(),    "Kanine");


/*

    let idx_node = Subject::new_kv(&context_b, "dummy","value").unwrap();
    idx_node.set_relation( 0, rec_b1 );

    println!("All rec_b1 MemoIds: {:?}", rec_b1_memoids);
    slab_b.remotize_memo_ids( &rec_b1_memoids );

    if let Some(record) = idx_node.get_relation(0) {
        println!("Retrieved record: {} - {:?}", record.id, record.get_value("animal_sound") );
    }

    let rec_b2 = Subject::new_kv(&context_a, "animal_sound","Meow");
    let rec_b3 = Subject::new_kv(&context_a, "animal_sound","Ribbit");

    rec_b2.set_relation( 1, rec_b1 );
    */

    // TODO: drop the referenced memos, ensuring we only have the remote memorefs Present
    // TODO: test relation changing/projection
    // TODO: fix/test subject reconstitution for relationship traversal (it's duping now)
    // TODO: build the index class using this primative
    // TODO: figure out how to bootstrap subject index, given that the subject index
    //       needs a (probably lesser) subject index to locate its index nodes

    //rec_b1.drop();

/*
    // Time moves forward
    net.deliver_all_memos();

    let rec_b1 = context_b.get_subject_by_id( rec_a1.id );
    assert!(rec_b1.is_ok(), "new subject should now be available on slab B");
    let rec_b1 = rec_b1.unwrap();

    assert!(rec_b1.get_value("animal_sound").unwrap() == "moo", "Transferred subject should be consistent");


    // Time moves forward
    net.deliver_all_memos();

    let rec_c1 = context_c.get_subject_by_id( rec_a1.id );
    assert!(rec_c1.is_ok(), "new subject should now be available on slab C");
    let mut rec_c1 = rec_c1.unwrap();

    assert!(rec_c1.get_value("animal_sound").unwrap() == "moo", "Transferred subject should be consistent");

    // Time moves forward
    net.deliver_all_memos();

    assert!( rec_c1.set_value("animal_sound", "woof"), "Change the value on slab C" );
    assert!( rec_c1.get_value("animal_sound").unwrap() == "woof", "Updated subject should be consistent");

    assert!( rec_a1.get_value("animal_sound").unwrap() == "moo", "Value should be unchanged on slab A" );

    // Time moves forward
    net.deliver_all_memos();

    assert!( rec_a1.get_value("animal_sound").unwrap() == "woof", "Now the value should be changed on slab A" );
*/

}
