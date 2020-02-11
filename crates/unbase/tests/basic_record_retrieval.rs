extern crate unbase;
use unbase::{
    util::simulator::Simulator,
    Entity,
    Network,
    Slab,
};

use tracing::debug;

#[unbase_test_util::async_test]
async fn basic_record_retrieval() {
    unbase_test_util::init_test_logger();

    let net = Network::create_new_system();
    let slab_a = Slab::initialize(&net);
    let context_a = slab_a.create_context();

    let record_id;
    {
        let record = Entity::new_with_single_kv(&context_a, "animal_type", "Cat").await.unwrap();

        debug!("Record {:?}", record);
        record_id = record.id;
    }

    let record_retrieved = context_a.get_entity_by_id(record_id).await.unwrap();

    assert!(record_retrieved.is_some(), "Failed to retrieve record")
}

#[unbase_test_util::async_test]
async fn basic_record_retrieval_simulator() {
    unbase_test_util::init_test_logger();

    let net = Network::create_new_system();
    let simulator = Simulator::new();
    net.add_transport(Box::new(simulator.clone()));

    let slab_a = Slab::initialize(&net);
    let context_a = slab_a.create_context();

    let record_id;
    {
        let record = Entity::new_with_single_kv(&context_a, "animal_type", "Cat").await.unwrap();

        debug!("Record {:?}", record);
        record_id = record.id;
    }

    let record_retrieved = context_a.get_entity_by_id(record_id).await.expect("retrieval");

    assert!(record_retrieved.is_some(), "Failed to retrieve record")
}
