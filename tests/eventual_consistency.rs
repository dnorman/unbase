#![feature(proc_macro, conservative_impl_trait, generators)]

extern crate unbase;
extern crate futures_await as futures;

use std::thread;
use unbase::SubjectHandle;
use futures::stream::Stream;
use std::sync::{Arc,Mutex};

#[test]
fn eventual_basic() {

    let net = unbase::Network::create_new_system();
    let mut simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let context_a = unbase::Slab::new(&net).create_context();
    let context_b = unbase::Slab::new(&net).create_context();

    let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo").expect("Subject A1");
    let record_id = rec_a1.id;

    assert!( rec_a1.get_value("animal_sound").unwrap() == "Moo", "New subject should be internally consistent");
    assert!( context_b.get_subject_by_id( record_id ).unwrap().is_none(), "new subject should not yet have conveyed to slab B");

    simulator.metronome(10);
    simulator.wait_ticks(2);

    let rec_b1 = context_b.get_subject_by_id( record_id ).unwrap();
    assert!(rec_b1.is_some(), "new subject should now have conveyed to slab B");

    let rec_b1 = rec_b1.unwrap();

    rec_b1.set_value("animal_sound","Woof").unwrap();

    simulator.wait_ticks(5);

    assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Woof");
}


#[test]
fn eventual_detail() {

    let net = unbase::Network::create_new_system();
    let mut simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );
    
    simulator.metronome(10);

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);
    let slab_c = unbase::Slab::new(&net);


    simulator.wait_ticks(3);

    assert!(slab_a.id == 0, "Slab A ID shoud be 0");
    assert!(slab_b.id == 1, "Slab B ID shoud be 1");
    assert!(slab_c.id == 2, "Slab C ID shoud be 2");

    assert!(slab_a.peer_slab_count() == 2, "Slab A Should know two peers" );
    assert!(slab_b.peer_slab_count() == 2, "Slab B Should know two peers" );
    assert!(slab_c.peer_slab_count() == 2, "Slab C Should know two peers" );

    let context_a = slab_a.create_context();
    let context_b = slab_b.create_context();
    let context_c = slab_c.create_context();

    simulator.pause();

    let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Moo");

    assert!(rec_a1.is_ok(), "New subject should be created");
    let rec_a1 = rec_a1.unwrap();

    assert!(rec_a1.get_value("animal_sound").unwrap() == "Moo", "New subject should be internally consistent");

    //println!("New subject ID {}", rec_a1.id );

    let record_id = rec_a1.id;
    let root_index_subject = if let Some(ref s) = *context_a.root_index.read().unwrap() {
        s.get_root_subject_handle(&context_a).unwrap()
    }else{
        panic!("sanity error - uninitialized context");
    };


    assert!(context_b.get_subject_by_id( record_id ).unwrap().is_none(), "new subject should not yet have conveyed to slab B");
    assert!(context_c.get_subject_by_id( record_id ).unwrap().is_none(), "new subject should not yet have conveyed to slab C");
    assert_eq!(
        (
            context_a.get_resident_subject_head_memo_ids(root_index_subject.id).len(),
            context_b.get_resident_subject_head_memo_ids(root_index_subject.id).len()
        ), (1,0), "Context A should  be seeded with the root index, and B should not" );

    simulator.resume();
    simulator.wait_ticks(5);

    assert_eq!(context_b.get_resident_subject_head_memo_ids(root_index_subject.id).len(), 1, "Context b should now be seeded with the root index" );


    let rec_b1 = context_b.get_subject_by_id( record_id ).unwrap();
    let rec_c1 = context_c.get_subject_by_id( record_id ).unwrap();

    //println!("RID: {}", record_id);
    assert!(rec_b1.is_some(), "new subject should now have conveyed to slab B");
    assert!(rec_c1.is_some(), "new subject should now have conveyed to slab C");

    let rec_b1 = rec_b1.unwrap();
    let rec_c1 = rec_c1.unwrap();

    let rec_c1_clone = rec_c1.clone();

    let last_observed_sound_c = Arc::new(Mutex::new(String::new()));
    
    {
        let last_observed_sound_c = last_observed_sound_c.clone();
        thread::spawn(move || {
            let stream = rec_c1_clone.observe();
            for _ in stream.wait() {
                let sound = rec_c1_clone.get_value("animal_sound").unwrap();
                *last_observed_sound_c.lock().unwrap() = sound.clone();
                //println!("rec_c1 changed. animal_sound is {}", sound );
            }
        });
    }

    simulator.wait_ticks(5);

    assert!(rec_b1.get_value("animal_sound").unwrap() == "Moo", "Subject read from Slab B should be internally consistent");
    assert!(rec_c1.get_value("animal_sound").unwrap() == "Moo", "Subject read from Slab C should be internally consistent");

    assert_eq!(rec_a1.get_value("animal_sound").unwrap(), "Moo");
    assert_eq!(rec_b1.get_value("animal_sound").unwrap(), "Moo");
    assert_eq!(rec_c1.get_value("animal_sound").unwrap(), "Moo");

    rec_b1.set_value("animal_type","Bovine").unwrap();
    assert_eq!(rec_b1.get_value("animal_type").unwrap(), "Bovine");
    assert_eq!(rec_b1.get_value("animal_sound").unwrap(),   "Moo");

    assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Moo");

    rec_b1.set_value("animal_sound","Woof").unwrap();
    rec_b1.set_value("animal_type","Kanine").unwrap();
    assert_eq!(rec_b1.get_value("animal_sound").unwrap(), "Woof");
    assert_eq!(rec_b1.get_value("animal_type").unwrap(),  "Kanine");

    simulator.wait_ticks(5);

    // Nowwww it should have propagated
    let expected_contents = ["I9001>I9003", "I9003>I9004", "I9004>I9005", "I9005>_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,I9006", "I9006>_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,_,R9002"];
    assert_eq!( context_a.concise_contents(),  &expected_contents );
    assert_eq!( context_b.concise_contents(), &expected_contents );

    assert_eq!(*last_observed_sound_c.lock().unwrap(),      "Woof");
    assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Woof");
    assert_eq!(rec_a1.get_value("animal_type").unwrap(),    "Kanine");

}
