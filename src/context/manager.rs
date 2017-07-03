

/// Graph datastore for MemoRefHeads in our query context.
///
/// New gameplan:
/// Now that we know we only wish to include index (tree structured) data, do we really need refcounts? Probably not
/// Yay! We're a DAG again ( probably. Need to think about B-tree rebalancing scenarios )
/// * Presumably we can return to more traditional toposort
/// * Error on addition of a cycle to the context
///
/// The functions of ContextManager are twofold:
/// 1. Store Subject MemoRefHeads to which an observer `Context` is actually contextualized. This is in turn used
/// by relationship projection logic (Context.get_subject_with_head calls ContextManager.get_head) 
/// 2. Provide a (cyclic capable) dependency iterator over the MemoRefHeads present, sufficient for context compression.
///    This is similar to, but not quite the same as a topological sort.
///
/// Crucially: `ContextManager` *must* be able to contain, and iterate cyclic subject relations. The underlying Memo structure is immutable,
/// and thus acyclic, but cyclic `Subject` relations are permissable due to what could be thought of as the "uncommitted" content of the ContextManager.
/// For this reason, we must perform context compression as a single pass over the contained MemoRefHeads, as a cyclic relation could otherwise cause and infinite loop.
/// The internal reference count increment/decrement thus contains cycle breaker functionality.
///
/// The ContextManager has been authored with the idea in mind that it generally should not exceed O(1k) MemoRefHeads.
/// It should be periodically compressed to control expansion, as its entire contents must be serialized for conveyance to neighbors.
///
/// Compression - a key requirement
/// 
/// Subject heads are applied to the context for any edit for which the consistency model enforcement is applicable.
/// Subject heads shall only removed from the context if:
/// * One or more other resident subject heads reference it
/// * All of the aforementioned subject heads descend it
///
/// Combined with index traversal, this meets our consistency invariant:
/// Projections done on the basis of an index or other relationship traversal shall include all edits whose heads were
/// added to the context previously.
/// Gradually, all edits will be rolled up into the index, which will always have at least one entry in the context (the root index node)
/// This has some crucial requirements:
/// 1. Non-index Subjects may not reference index subjects, or at least these references must not count
///    toward compaction rules
/// 2. Relationship traversals must specify which they require:
///    A. Relative consistency with the record from which they originated
///    B. Absolute consistency with the query context, which likely requires a supplimental index traversal.
///    Index subjects themselves would require only the former of course (with the latter being impossible)
///    For non-index relationship traversals, this is potentially ornerous from a performance perspective)
///
/// Invariant 1: The each state of the context manager must be descendent of its prior state
///  
impl ContextManager {




    pub fn get_edit_counter(&self) -> usize {
        self.inner.lock().unwrap().edit_counter
    }
    /// Get MemoRefHead (if resident) for the provided subject_id
    pub fn get_head(&mut self, subject_id: SubjectId) -> Option<MemoRefHead> {
        let inner = self.inner.lock().unwrap();

        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(ContextItem{ head: Some(ref h), .. })) => {
                        Some(h.clone())
                    },
                    None => None
                }
            }
            None => None
        }
    }
    
    pub fn subject_head_iter (&self) -> SubjectHeadIter {
        let inner = self.inner.lock().unwrap();
        inner.iter_counter += 1;

        SubjectHeadIter::new(self.clone())
    }
    fn increment(&mut self, item_id: ItemId, increment: isize, seen: &mut Vec<bool>) {
        // Avoid traversing cycles
        if Some(&true) == seen.get(item_id) {
            return; // dejavu! Bail out
        }
        seen[item_id] = true;

        let relations: Vec<ItemId>;
        let mut remove = false;
        {
            if let &mut Some(ref mut item) = &mut self.items[item_id] {
                item.indirect_references += increment;
                if item.indirect_references == 0 && item.head.is_none(){
                    remove = true;
                }
                assert!(item.indirect_references >= 0,
                        "sanity error. indirect_references below zero");

                relations = item.relations.iter().filter_map(|r| *r).collect();
            } else {
                panic!("sanity error. increment for item_id");
            }
        };

        if remove {
            self.items[item_id] = None;
            self.vacancies.push(item_id);
        }

        for rel_item_id in relations {
            self.increment(rel_item_id, increment, seen);
        }

    }
    // I don't really like have this be internal to the manager, but there's presently no item_id based interface and I'm not sure if there will be.
    // It's inefficient to have to convert back and forth between SubjectId and ItemId.
    pub fn compress(&mut self, slab: &Slab) {

        unimplemented!();
        // for subject_head in self.subject_head_iter() {
        //     // TODO: think about whether we should limit ourselves to doing this only for resident subjects with heads

        //     if let Some(ref item) = self.items[subject_head.item_id] {
        //         let mut rssh = RelationSlotSubjectHead::empty();

        //         for (slot_id, maybe_rel_item_id) in item.relations.iter().enumerate(){
        //             if let Some(rel_item_id) = *maybe_rel_item_id {
        //                 if let Some(ref mut rel_item) = self.items[rel_item_id] {
        //                     let decrement = 0 - (item.indirect_references + 1);
        //                     if let Some(head) = rel_item.head.take() {
        //                         rssh.insert(slot_id as RelationSlotId, rel_item.subject_id, head);
                                
        //                         let mut removed = vec![false; self.items.len()];
        //                         self.increment(rel_item_id, decrement, &mut removed);
        //                     }
        //                 }
        //             }
        //         }

        //         if rssh.len() > 0 {
        //             let memoref = slab.new_memo(
        //                 Some(subject_head.subject_id),
        //                 subject_head.head.clone(),
        //                 MemoBody::Relation( rssh )
        //             );

        //             let new_head = memoref.to_head();

        //             let relation_links = new_head.project_all_relation_links(&slab);
        //             self.apply_head( subject_head.subject_id, relation_links , new_head );

        //         }
        //     }
        // }
    }
}

impl ContextManagerInner {
    /// Creates or returns a ContextManager item for a given subject_id
    fn assert_item(&mut self, subject_id: SubjectId) -> ItemId {

        let index = &mut self.index;
        match index.binary_search_by(|x| x.0.cmp(&subject_id) ){
            Ok(i) => {
                index[i].1
            }
            Err(i) =>{
                let item = ContextItem::new(subject_id, None);

                let item_id = if let Some(item_id) = self.vacancies.pop() {
                    self.items[item_id] = Some(item);
                    item_id
                } else {
                    self.items.push(Some(item));
                    self.items.len() - 1
                };

                index.insert(i, (subject_id, item_id) );
                item_id
            }
        }
    }
    fn increment_item(&mut self, item: &ContextItem){
        unimplemented!();
    }
    fn decrement_item(&mut self, item: &ContextItem) {
        unimplemented!();
    }
    fn increment_descendents (&mut self, item: &ContextItem){
        unimplemented!();
    }
    fn remove_head (&self, subject_id: SubjectId) {
        unimplemented!();
        // let inner = self.inner.lock().unwrap();

        // if *inner.iter_counter == 0 {
        //     inner.vacancies.push(item_id);
        // }else{
        //     // avoid use after free until all iters have finished
        //     // we don't have to guarantee that the item sticks around, just that we don't give the iter the wrong item
        //     inner.pending_vacancies.push(item_id);
        // }
    }
}

//impl ContextManager {

    /// For a given SubjectId, Retrieve a RelationSlotSubjectHead containing all referred subject heads resident in the `ContextManager`
    // pub fn compress_subject(&mut self, subject_id: SubjectId) -> Option<RelationSlotSubjectHead> {
    //     let mut slot_item_ids = Vec::new();
    //     {
    //         if let Some(ref mut item) = self.get_item_by_subject( subject_id ) {
    //             for (slot_id, maybe_rel_item_id) in item.relations.iter().enumerate(){
    //                 if let Some(rel_item_id) = *maybe_rel_item_id {
    //                     slot_item_ids.push((slot_id,rel_item_id));
    //                 }
    //             }
    //         }
    //     }
        
    //     if slot_item_ids.len() > 0 {

    //         let mut rssh = RelationSlotSubjectHead::empty();

    //         for (slot_id,rel_item_id) in slot_item_ids {
    //             if let Some(ref mut rel_item) = self.items[rel_item_id] {
    //                 if let Some(ref head) = rel_item.head {
    //                     rssh.insert(slot_id as RelationSlotId, rel_item.subject_id, head.clone());
    //                 }
    //             }
    //         }
    //         Some(rssh)
    //     }else {
    //         None
    //     }
    // }

    // // starting context: [C <- B <- A, X]
    // // Materialize C if needed, then replace C
    // // Materialize B to point to C, then replace B and remove C
    // // Materialize A to point to B, then replace A and remove B
    // // End context [A, X]
    // pub fn compress_and_materialize(&self) {


    //     // TODO: conditionalize this on the basis of the present context size

    //     let parent_repoints : HashMap<SubjectId,RelationSlotSubjectHead>
    //     // Iterate the contextualized subject heads in reverse topological order
    //     for subject_head in {
    //         self.manage.subject_head_iter()
    //     } {

    //         // TODO: implement MemoRefHead.conditionally_materialize such that the materialization threshold is selected dynamically.
    //         //       It shold almost certainly not materialize with a single edit since the last FullyMaterialized memo
    //         // head.conditionally_materialize( &self.slab );

    //         if subject_head.from_subject_ids.len() > 0 {
    //             // OK, somebody is pointing to us, so lets issue an edit for them
    //             // to point to the new materialized memo for their relevant relations


    //             for (from_head) in subject_head.referring_heads.iter(){

    //                 let memoref = self.slab.new_memo(
    //                     Some(self.id),
    //                     head.clone(),
    //                     MemoBody::Relation(RelationSlotSubjectHead(memoref_map))
    //                 );

    //                 head.apply_memoref(&memoref, &slab);
    //             }

    //            self.repoint_subject_relations(subject_head.subject_id,
    //                                            subject_head.head,
    //                                            subject_head.from_subject_ids);

    //             // NOTE: In order to remove a subject head from the context, we must ensure that
    //             //       ALL referencing subject heads in the context get repointed. It's not enough to just do one

    //             // Now that we know they are pointing to the new materialized MemoRefHead,
    //             // and that the resident subject struct we have is already updated, we can
    //             // remove this subject MemoRefHead from the context head, because subsequent
    //             // index/graph traversals should find this updated parent.
    //             //
    //             // When trying to materialize/compress fully (not that we'll want to do this often),
    //             // this would continue all the way to the root index node, and we should be left
    //             // with a very small context head

    //         }
    //     }

    // }
//}


pub struct SubjectHeadReport {
    item_id: ItemId,
    pub subject_id: SubjectId,
    pub head: MemoRefHead,
    pub from_subject_ids: Vec<SubjectId>,
    pub to_subject_ids: Vec<SubjectId>,
    pub indirect_references: usize,
}

pub struct SubjectHeadIter {
    manager: ContextManager,
    edit_counter: usize,
    items: Vec<(bool,usize,ItemId)>, // visited, refcounts, item_id
    visited: Vec<ItemId>
}
impl Iterator for SubjectHeadIter {
    type Item = SubjectHeadReport;

    fn next(&mut self) -> Option<Self::Item> {
        // Game plan - We're going to start with the brute force way.
        //             see fn calculate for details.
        //             snag a copy of the manager edit_counter so we can recalculate if any edits have been made while our iterator is active

        let mgr_ec = self.manager.get_edit_counter();
        if mgr_ec != self.edit_counter {
            self.calculate();
            self.edit_counter = mgr_ec;
        }

        loop{
            // get next item from the non_visited calculated_refcounts
            // be prepared 
        }

        unimplemented!()
    }
}

/// Reverse topological iterator over subject heads which are resident in the context manager
impl SubjectHeadIter {
    fn new(manager: ContextManager) -> Self {
        unimplemented!()
        
        // SubjectHeadIter{
        //     manager: manager,
        //     edit_counter: !0,
        //     items: Vec::new()
        // }
    }
    pub fn get_subject_ids (&mut self) -> Vec<SubjectId> {
        self.map(|x| x.subject_id ).collect()
    }
    fn calculate(&mut self) {
        // 0. Make a note of the edit counter for the graph, cache the below for as long as it is unchanged
        // 1. Generate inverse adjacency list for whole graph
        // 2. Iterate over ALL vertices in the graph, using each as the starting point for a DFS
        // 3. count all indirect references for each DFS source vertex (IE: all of them)
        // 4. order vertices by indirect reference count descending
        // 5. skip any that we've visited already
        // 6. recalculate if the edit counter has incremented

        // Note: when recalculating, it's fine to overwrite the refcounts, but do not overwrite the visited flag

        unimplemented!()
    }

    // fn increment(&mut self, item_id: ItemId, increment: isize, seen: &mut Vec<bool>) {
    //     // Avoid traversing cycles
    //     if Some(&true) == seen.get(item_id) {
    //         return; // dejavu! Bail out
    //     }
    //     seen[item_id] = true;

    //     let relations: Vec<ItemId>;
    //     let mut remove = false;
    //     {
    //         if let &mut Some(ref mut item) = &mut self.items[item_id] {
    //             item.indirect_references += increment;
    //             if item.indirect_references == 0 && item.head.is_none(){
    //                 remove = true;
    //             }
    //             assert!(item.indirect_references >= 0,
    //                     "sanity error. indirect_references below zero");

    //             relations = item.relations.iter().filter_map(|r| *r).collect();
    //         } else {
    //             panic!("sanity error. increment for item_id");
    //         }
    //     };

    //     if remove {
    //         self.items[item_id] = None;
    //         self.vacancies.push(item_id);
    //     }

    //     for rel_item_id in relations {
    //         self.increment(rel_item_id, increment, seen);
    //     }

    // }
}

