    //println!("Manually exchanging context from Context B to Context A - Count of MemoRefs: {}", context_b.hack_send_context(&context_a) );

    // Force compaction of the index to ensure a new root index node is issued.
    // In the future, this will occur automatically on the basis of a probablistic interval.
    // This will likely be the primary eventual consistency mechanisim for convergence
    // context_a.compact();
    // context_a.compact(); // HACK - run it a couple times, because it's not performing a topologically optimal compaction yet
    // context_a.compact();

    // Temporary way to magically, instantly send context
    // println!("Manually exchanging context from Context A to Context B - Count of MemoRefs: {}", context_a.hack_send_context(&context_b) );
    // println!("Manually exchanging context from Context A to Context C - Count of MemoRefs: {}", context_a.hack_send_context(&context_c) );
