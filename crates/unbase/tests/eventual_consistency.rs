use futures::{
    future::RemoteHandle,
    StreamExt,
};
use std::sync::{
    Arc,
    Mutex,
};
use unbase::{
    util::{
        simulator::Simulator,
        task::spawn_with_handle,
    },
    Entity,
    Network,
    Slab,
};

use tracing::debug;

#[unbase_test_util::async_test]
async fn eventual_basic() {
    let net = Network::create_new_system();
    let simulator = Simulator::new();
    net.add_transport(Box::new(simulator.clone()));

    simulator.start();

    let slab_a = Slab::initialize(&net);
    let slab_b = Slab::initialize(&net);
    let context_a = slab_a.create_context();
    let context_b = slab_b.create_context();

    let mut rec_a1 = Entity::new_with_single_kv(&context_a, "animal_sound", "Moo").await
                                                                                  .expect("Entity A1");
    let record_id = rec_a1.id;

    assert!(rec_a1.get_value("animal_sound").await.unwrap().unwrap() == "Moo",
            "New entity should be internally consistent");
    assert!(context_b.get_entity_by_id(record_id).await.unwrap().is_none(),
            "new entity should not yet have conveyed to slab B");

    simulator.quiesce().await;

    let rec_b1 = context_b.get_entity_by_id(record_id).await.unwrap();
    assert!(rec_b1.is_some(), "new entity should now have conveyed to slab B");

    let mut rec_b1 = rec_b1.unwrap();

    rec_b1.set_value("animal_sound", "Woof").await.unwrap();

    simulator.quiesce().await;

    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Woof");
}

#[unbase_test_util::async_test]
async fn eventual_detail() {
    let net = Network::create_new_system();
    let simulator = Simulator::new();
    net.add_transport(Box::new(simulator.clone()));

    simulator.start();

    let slab_a = Slab::initialize(&net);
    let slab_b = Slab::initialize(&net);
    let slab_c = Slab::initialize(&net);

    simulator.quiesce().await;

    assert!(slab_a.id == 0, "Slab A ID shoud be 0");
    assert!(slab_b.id == 1, "Slab B ID shoud be 1");
    assert!(slab_c.id == 2, "Slab C ID shoud be 2");

    assert!(slab_a.peer_slab_count() == 2, "Slab A Should know two peers");
    assert!(slab_b.peer_slab_count() == 2, "Slab B Should know two peers");
    assert!(slab_c.peer_slab_count() == 2, "Slab C Should know two peers");

    let context_a = slab_a.create_context();
    let context_b = slab_b.create_context();
    let context_c = slab_c.create_context();

    simulator.quiesce_and_stop().await;

    let rec_a1 = Entity::new_with_single_kv(&context_a, "animal_sound", "Moo").await;

    assert!(rec_a1.is_ok(), "New entity should be created");
    let mut rec_a1 = rec_a1.unwrap();

    assert!(rec_a1.get_value("animal_sound").await.unwrap().unwrap() == "Moo",
            "New entity should be internally consistent");

    // println!("New entity ID {}", rec_a1.id );

    let record_id = rec_a1.id;
    let root_index = context_a.root_index().await.unwrap();

    assert!(context_b.get_entity_by_id(record_id).await.unwrap().is_none(),
            "new entity should not yet have conveyed to slab B");
    assert!(context_c.get_entity_by_id(record_id).await.unwrap().is_none(),
            "new entity should not yet have conveyed to slab C");

    // Not sure how this formerly worked, but I can see no reason why the context wouldnt have the root index head,
    // since it gets it directly from Network. Something has clearly changed in the merge, but I'm not sure what
    //    assert_eq!(
    //        (
    //            context_a.get_resident_entity_head_memo_ids(root_index.get_root_entity_id() ).len(),
    //            context_b.get_resident_entity_head_memo_ids(root_index.get_root_entity_id() ).len()
    //        ), (1,0), "Context A should be seeded with the root index, and B should not" );

    simulator.start();
    simulator.quiesce().await;

    assert_eq!(context_b.get_resident_entity_head_memo_ids(root_index.get_root_entity_id())
                        .len(),
               1,
               "Context b should now be seeded with the root index");

    let rec_b1 = context_b.get_entity_by_id(record_id).await.unwrap();
    let rec_c1 = context_c.get_entity_by_id(record_id).await.unwrap();

    // println!("RID: {}", record_id);
    assert!(rec_b1.is_some(), "new entity should now have conveyed to slab B");
    assert!(rec_c1.is_some(), "new entity should now have conveyed to slab C");

    let mut rec_b1 = rec_b1.unwrap();
    let mut rec_c1 = rec_c1.unwrap();

    async fn update_last_observed_sound(tmp: Arc<Mutex<String>>, mut rec: Entity) {
        let mut stream = rec.observe();
        while let Some(_head) = stream.next().await {
            let sound = rec.get_value("animal_sound").await.unwrap().unwrap();
            *tmp.lock().unwrap() = sound.clone();
            debug!("rec_c1 changed. animal_sound is {}", sound);
        }
    }

    let last_observed_sound_c = Arc::new(Mutex::new(String::new()));
    let _applier: RemoteHandle<()> = spawn_with_handle(update_last_observed_sound(last_observed_sound_c.clone(), rec_c1.clone()));

    simulator.quiesce().await;

    assert!(rec_b1.get_value("animal_sound").await.unwrap().unwrap() == "Moo",
            "Entity read from Slab B should be internally consistent");
    assert!(rec_c1.get_value("animal_sound").await.unwrap().unwrap() == "Moo",
            "Entity read from Slab C should be internally consistent");

    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");
    assert_eq!(rec_b1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");
    assert_eq!(rec_c1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");

    rec_b1.set_value("animal_type", "Bovine").await.unwrap();
    assert_eq!(rec_b1.get_value("animal_type").await.unwrap().unwrap(), "Bovine");
    assert_eq!(rec_b1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");

    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Moo");

    rec_b1.set_value("animal_sound", "Woof").await.unwrap();
    rec_b1.set_value("animal_type", "Kanine").await.unwrap();
    assert_eq!(rec_b1.get_value("animal_sound").await.unwrap().unwrap(), "Woof");
    assert_eq!(rec_b1.get_value("animal_type").await.unwrap().unwrap(), "Kanine");

    // TODO add wait_ticks back to simulator
    //    simulator.wait_ticks(5);
    simulator.quiesce().await;

    // Nowwww it should have propagated
    let expected_contents = "I9001>I9003;I9003>I9004;I9004>I9005;I9005>_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,\
                             _,_,_,_,_,_,_,I9006;I9006>_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,\
                             _,_,_,_,_,_,R9002";
    assert_eq!(context_a.concise_contents(), expected_contents);
    assert_eq!(context_b.concise_contents(), expected_contents);

    assert_eq!(*last_observed_sound_c.lock().unwrap(), "Woof");
    assert_eq!(rec_a1.get_value("animal_sound").await.unwrap().unwrap(), "Woof");
    assert_eq!(rec_a1.get_value("animal_type").await.unwrap().unwrap(), "Kanine");
}
