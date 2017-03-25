extern crate unbase;
use unbase::subject::Subject;
use std::{thread, time};

//#[test]
fn remote_traversal_simulated() {

    let net = unbase::Network::new();
    let simulator = unbase::network::transport::Simulator::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let context_a = slab_a.create_context();
    let _context_b = slab_b.create_context();

    let rec_a1 = Subject::new_kv(&context_a, "animal_sound", "Moo").unwrap();

    rec_a1.set_value("animal_sound","Woof");
    rec_a1.set_value("animal_sound","Meow");

    simulator.advance_clock(1); // Now it should have propagated to slab B

    simulator.advance_clock(1); // now slab A should know that Slab B has it

    slab_a.remotize_memo_ids( &rec_a1.get_all_memo_ids() );

    simulator.advance_clock(1);

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
fn avoid_unnecessary_chatter() {

    let net = unbase::Network::new();

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let _context_a = slab_a.create_context();
    let _context_b = slab_b.create_context();

    thread::sleep(time::Duration::from_millis(100));

    println!("Slab A MemoRefs present {}", slab_a.count_of_memorefs_resident() );
    println!("Slab A MemoRefs present {}", slab_b.count_of_memorefs_resident() );

    println!("Slab A Memos received {}", slab_a.count_of_memos_received() );
    println!("Slab B Memos received {}", slab_a.count_of_memos_received() );

}

fn remote_traversal_nondeterministic() {

    let net = unbase::Network::new();

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let context_a = slab_a.create_context();
    let _context_b = slab_b.create_context();

    //let rec_a1 = Subject::new_kv(&context_a, "animal_sound", "Moo").unwrap();

    //rec_a1.set_value("animal_sound","Woof");
    //rec_a1.set_value("animal_sound","Meow");

    thread::sleep(time::Duration::from_millis(5000));

    //slab_a.remotize_memo_ids( &rec_a1.get_all_memo_ids() );

    //thread::sleep(time::Duration::from_millis(5000));

    //let handle = thread::spawn(move || {

        //assert_eq!(rec_a1.get_value("animal_sound").unwrap(),   "Meow");

    //});

    //thread::sleep(time::Duration::from_millis(5000));

    //handle.join().unwrap();

}
