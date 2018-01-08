extern crate unbase;
use unbase::SubjectHandle;
use std::{thread, time};

fn main() {

    let simulator = unbase::network::transport::Simulator::new();
    let net = unbase::Network::new();
    net.add_transport( Box::new(simulator.clone()) );

    let slab_a = unbase::Slab::new(&net);
    let slab_b = unbase::Slab::new(&net);

    let context_a = slab_a.create_context();
    let context_b = slab_b.create_context();

    let rec_a1 = SubjectHandle::new_kv(&context_a, "animal_sound", "Meow").unwrap();
    let rec_id = rec_a1.id; // useful for cross-context retrieval

    // ************************************************************************
    // Create one record, then spawn two threads,
    // each of which makes an edit whenever it sees an edit

    // NOTE: have to use polling for now to detect when the subject has changed
    // because push notification (though planned) isn't implemented yet :)
    // ************************************************************************

    let half_sec = time::Duration::from_millis(500);
    let ten_ms = time::Duration::from_millis(10);

    // spawn thread 1
    let t1 = thread::spawn(move || {
        // use the original copy of the subject, or look it up by sub
        let rec_a1 = context_a.get_subject_by_id( rec_id ).unwrap().unwrap();

        for _ in 1..5 {
            // Hacky-polling approach for now, push notification coming sooooon!
            loop {
                if "Meow".to_string() == rec_a1.get_value("animal_sound").unwrap() {
                    // set a value when a change is detected

                    println!("[[[ Woof ]]]");
                    rec_a1.set_value("animal_sound","Woof");
                    break;
                }
                thread::sleep(ten_ms);
            }
        }
    });


    //

    // spawn thread 2
    let t2 = thread::spawn(move || {
        // cheater cheater! ( not yet a blocking version of get_subject )
        thread::sleep(ten_ms);

        // Get a new copy of the same subject from context_b (requires communication)
        let rec_b1 = context_b.get_subject_by_id( rec_id ).unwrap().unwrap();

        for _ in 1..5 {
            // Hacky-polling approach for now, push notification coming sooooon!
            loop {
                if "Woof".to_string() == rec_b1.get_value("animal_sound").unwrap() {
                    // set a value when a change is detected
                    println!("[[[ Meow ]]]");
                    rec_b1.set_value("animal_sound","Meow");
                    break;
                }
                thread::sleep(ten_ms);
            }
        }
    });

    for _ in 1..12 {
        simulator.advance_clock(1);
        thread::sleep(half_sec);
    }

    // Remember, we're simulating the universe here, so we have to tell the universe to continuously move forward

    t1.join().unwrap();
    t2.join().unwrap();

}
