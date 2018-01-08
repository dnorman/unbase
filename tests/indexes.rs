extern crate unbase;
use unbase::SubjectHandle;
use unbase::index::IndexFixed;

#[test]
fn index_construction() {

    let net = unbase::Network::create_new_system();
    let simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let context_a = slab_a.create_context();

    // Create a new fixed tier index (fancier indexes not necessary for the proof of concept)

    let index = IndexFixed::new(&context_a, 5);

    assert_eq!( context_a.is_fully_materialized(), true );

    // First lets do a single index test
    let i = 1234;
    let record = SubjectHandle::new_kv(&context_a, "record number", &format!("{}",i)).unwrap();
    index.insert_subject_handle(i, &record);

    assert_eq!( index.get_subject_handle(&context_a,1234).unwrap().unwrap().get_value("record number").unwrap(), "1234");


    // Ok, now lets torture it a little
    for i in 0..10 {
        let record = SubjectHandle::new_kv(&context_a, "record number", &format!("{}",i)).unwrap();
        index.insert_subject_handle(i, &record);
    }

    for i in 0..10 {
        assert_eq!( index.get_subject_handle(&context_a,i).unwrap().unwrap().get_value("record number").unwrap(), i.to_string() );
    }

    //assert_eq!( context_a.is_fully_materialized(), false );
    //context_a.fully_materialize();
}
