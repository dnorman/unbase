extern crate unbase;
use unbase::SubjectHandle;

#[test]
fn basic_record_retrieval() {

    let net = unbase::Network::create_new_system();
    let slab_a = unbase::slab::Memory::new(&net);
    let context_a = slab_a.create_context();

    let record_id;
    {
        let record = SubjectHandle::new_kv(&context_a, "animal_type","Cat").unwrap();

        println!("Record {:?}", record );
        record_id = record.id;
    }

    let record_retrieved = context_a.get_subject_by_id(record_id);

    assert!(record_retrieved.is_ok(), "Failed to retrieve record")

}
#[test]
fn basic_record_retrieval_simulator() {

    let net = unbase::Network::create_new_system();
    let simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let context_a = slab_a.create_context();

    let record_id;
    {
        let record = SubjectHandle::new_kv(&context_a, "animal_type","Cat").unwrap();

        println!("Record {:?}", record );
        record_id = record.id;
    }

    let record_retrieved = context_a.get_subject_by_id(record_id);

    assert!(record_retrieved.is_ok(), "Failed to retrieve record")

}
