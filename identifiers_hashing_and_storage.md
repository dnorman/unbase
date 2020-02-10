
* EntityId - ULID
   * Entities require enumeration, as they are a topic of ongoing "discussion"
   * Using ULID to encode more entropy more efficiently, and to enable better bucketization / lexicographic sorting
   * Also gives us a free timestamp for the creation of the Entity
* SlabId - ULID
* Memo "ID" (hash)
   What's the optimal way to decouple Memo from Head in order to accommodate their recursive storage,
   while limiting the complexity of the lookups we have to do?
   
   How does this differ between memory / disk / network addressing scenarios?
   
   We want to:
     * avoid hashing a memo for as long as possible
     * seek to avoid random access / non-lexacographic storage as much as possible
     * Provide the potential to theoretically avoid heap allocations in some cases (embedded systems)
     * Locate resources which are likely to be used together in physical proximity
     
   I am imagining something like:
    Box<Head<Link<Memo>>
    struct Link {
        memo: Memo, // Uh, Memo is also Boxed inside. Maybe Memo itself should be a linked list?
        next: Option<Box<Link>>
    }
    
    // embedded systems would replace Link with something like struct Fixed { one: Memo, two: Option<Memo> } and
    // deny concurrencies above 2
   
* Policy Key ( Private / public ) Policy authority needs to be able to sign policy directives

#### Re Memo/Memorefhead storage:
 * We will have to store Memos as Frames I think. Can't just be a big heap alloc (seek precedent here for storage paradigm)
 * What are the downsides of a best-effort-adjacency Arena-allocated linked list? (what is its degredation behavior?)
 * Investigate Unrolled linked lists ( which may be the same as the above )
 * Investigate B-Trees for this purpose
 * Investigate Ropes
 * Investigate CDR coding
 

#### Attack Vectors

What are the attack vectors here?

* Cybil
* DOS