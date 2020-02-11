use unbase::{
    util::simulator::Simulator,
    Entity,
    Network,
    Slab,
};

#[unbase_test_util::async_test]
async fn acyclic() {
    unbase_test_util::init_test_logger();

    let net = Network::create_new_system();
    let simulator = Simulator::new();
    net.add_transport(Box::new(simulator.clone()));

    let slab_a = Slab::initialize(&net);
    let context_a = slab_a.create_context();

    let record1 = Entity::new_blank(&context_a).await.unwrap();
    let mut record2 = Entity::new_blank(&context_a).await.unwrap();
    let mut record3 = Entity::new_blank(&context_a).await.unwrap();
    let mut record4 = Entity::new_blank(&context_a).await.unwrap();
    let mut record5 = Entity::new_blank(&context_a).await.unwrap();
    let mut record6 = Entity::new_blank(&context_a).await.unwrap();

    record2.set_relation(0, &record1).await.unwrap();
    record3.set_relation(0, &record1).await.unwrap();
    record4.set_relation(0, &record1).await.unwrap();
    record5.set_relation(0, &record2).await.unwrap();
    record6.set_relation(0, &record5).await.unwrap();
}
