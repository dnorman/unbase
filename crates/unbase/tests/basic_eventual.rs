use unbase::{
    util::simulator::Simulator,
    Entity,
    Network,
    Slab,
};

use tracing::debug;

#[unbase_test_util::async_test]
async fn basic_eventual() {
    unbase_test_util::init_test_logger();

    // initialize the system, and use the simulator transport
    let net = Network::create_new_system();
    let simulator = Simulator::new();
    net.add_transport(Box::new(simulator.clone()));

    // Set up three Slabs, corresponding to three hosts, or three OS processes, or MAYBE even three different workers
    // within the same OS process
    let slab_a = Slab::initialize(&net);
    let slab_b = Slab::initialize(&net);
    let slab_c = Slab::initialize(&net);

    // Basic sanity tests
    assert!(slab_a.id == 0, "Slab A ID shoud be 0");
    assert!(slab_b.id == 1, "Slab B ID shoud be 1");
    assert!(slab_c.id == 2, "Slab C ID shoud be 2");

    assert!(slab_a.peer_slab_count() == 2, "Slab A Should know two peers");
    assert!(slab_b.peer_slab_count() == 2, "Slab B Should know two peers");
    assert!(slab_c.peer_slab_count() == 2, "Slab C Should know two peers");

    // Spawns a task to tick away automatically in the background
    simulator.start();

    let context_a = slab_a.create_context();
    let mut context_b = slab_b.create_context();
    let mut context_c = slab_c.create_context();

    let rec_a1 = Entity::new_with_single_kv(&context_a, "animal_sound", "Moo").await;
    assert!(rec_a1.is_ok(), "New entity should be created");
    let mut rec_a1 = rec_a1.unwrap();

    assert!(rec_a1.get_value("animal_sound").await.expect("retrieval").expect("has value") == "Moo",
            "New entity should be internally consistent");

    // TODO: consolidation is necessary for eventual consistency to work
    // context_a.fully_consolidate();

    debug!("New entity ID {}", rec_a1.id);

    let record_id = rec_a1.id;

    // TODO: move this to another test
    //    let root_index_entity_id = if let Some(ref s) = context_a.root_index().unwrap() {
    //        s.get_root_id()
    //    }else{
    //        panic!("sanity error - uninitialized context");
    //    };

    assert!(context_b.get_entity_by_id(record_id)
                     .await
                     .expect("query succeeded")
                     .is_none(),
            "new entity should not yet have conveyed to slab B");
    assert!(context_c.get_entity_by_id(record_id)
                     .await
                     .expect("query succeeded")
                     .is_none(),
            "new entity should not yet have conveyed to slab C");

    simulator.quiesce().await;

    //    debug!("Root Index = {:?}", context_b.get_resident_entity_head_memo_ids(root_index_entity_id)  );

    // TODO: replace this – Temporary way to magically, instantly send context
    debug!("Manually exchanging context from Context A to Context B - Count of MemoRefs: {}",
           context_a.hack_send_context(&mut context_b).await.expect("it worked"));
    debug!("Manually exchanging context from Context A to Context C - Count of MemoRefs: {}",
           context_a.hack_send_context(&mut context_c).await.expect("it worked"));
    //    debug!("Root Index = {:?}", context_b.get_entity_head_memo_ids(root_index_entity_id)  );

    let rec_b1 = context_b.get_entity_by_id(record_id).await.expect("it worked");
    let rec_c1 = context_c.get_entity_by_id(record_id).await.expect("it worked");

    assert!(rec_b1.is_some(), "new entity should now have conveyed to slab B");
    assert!(rec_c1.is_some(), "new entity should now have conveyed to slab C");

    let mut rec_b1 = rec_b1.expect("found");
    let mut rec_c1 = rec_c1.unwrap();

    assert!(rec_b1.get_value("animal_sound").await.unwrap().unwrap() == "Moo",
            "Entity read from Slab B should be internally consistent");
    assert!(rec_c1.get_value("animal_sound").await.unwrap().unwrap() == "Moo",
            "Entity read from Slab C should be internally consistent");

    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");
    assert_eq!(rec_b1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");
    assert_eq!(rec_c1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");

    // Now lets make some changes

    rec_b1.set_value("animal_type", "Bovine").await.unwrap();
    assert_eq!(rec_b1.get_value("animal_type").await.unwrap().unwrap(), "Bovine");
    assert_eq!(rec_b1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");

    rec_b1.set_value("animal_sound", "Woof").await.unwrap();
    rec_b1.set_value("animal_type", "Kanine").await.unwrap();
    assert_eq!(rec_b1.get_value("animal_sound").await.unwrap().unwrap(), "Woof");
    assert_eq!(rec_b1.get_value("animal_type").await.unwrap().unwrap(), "Kanine");

    // Should not yet have propagated to slab A
    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");
    assert_eq!(rec_a1.get_value("animal_type").await.unwrap(),
               None,
               "Should not yet have a value on Slab A for animal_type");

    simulator.start(); // advance the simulator clock by one tick
    simulator.quiesce().await;

    // Nowwww it should have propagated
    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Woof");
    assert_eq!(rec_a1.get_value("animal_type").await.unwrap().unwrap(), "Kanine");

    simulator.quiesce_and_stop().await;
}
