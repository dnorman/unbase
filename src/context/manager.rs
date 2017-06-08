
use super::*;
use memorefhead::{RelationSlotId,RelationLink};
use std::sync::{Arc,Mutex,RwLock};

type ItemId = usize;

struct ContextItem {
    subject_id:   SubjectId,
    refcount:     usize,
    head:         Option<MemoRefHead>,
    relations:    Vec<Option<ItemId>>,
    edit_counter: usize,
}

/// Performs topological sorting.
pub struct ContextManager {
    inner:      Mutex<ContextManagerInner>,
    //pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}
pub struct ContextManagerInner{
    items:             Vec<Option<ContextItem>>,
    index:             Vec<(SubjectId,ItemId)>,
    vacancies:         Vec<ItemId>,
    pending_vacancies: Vec<ItemId>,
    iter_counter:      usize,
    edit_counter:      usize
}

impl ContextItem {
    fn new(subject_id: SubjectId, maybe_head: Option<MemoRefHead>) -> Self {
        ContextItem {
            subject_id: subject_id,
            head: maybe_head,
            refcount: 0,
            relations: Vec::new(),
            edit_counter: 0
        }
    }
}

/// Graph datastore for MemoRefHeads in our query context.
///
/// The functions of ContextManager are twofold:
/// 1. Store Subject MemoRefHeads to which an observe `Context` is actually contextualized. This is in turn used
/// by relationship projection logic (Context.get_subject_with_head calls ContextManager.get_head) 
/// 2. Provide a (cyclic capable) dependency iterator over the MemoRefHeads present, sufficient for context compression.
///    This is similar to, but not quite the same as a topological sort.
///
/// Crucially: `ContextManager` *must* be able to contain, and iterate cyclic subject relations. The underlying Memo structure is immutable,
/// and thus acyclic, but cyclic `Subject` relations are permissable due to what could be thought of as the "uncommitted" content of the ContextManager.
/// For this reason, we must perform context compression as a single pass over the contained MemoRefHeads, as a cyclic relation could otherwise cause and infinite loop.
/// The internal reference count increment/decrement thus contains cycle breaker functionality.
///
/// The ContextManager has been authored with the idea in mind that it generally should not exceed O(1k) MemoRefHeads. It should be periodically compressed to control expansion.
impl ContextManager {
    pub fn new() -> ContextManager {
        ContextManager {
            inner: Mutex::new(ContextManagerInner{
                items:        Vec::with_capacity(30),
                index:        Vec::with_capacity(30),
                vacancies:    Vec::with_capacity(30),
                pending_vacancies: Vec::with_capacity(30),
                edit_counter: 0,
                iter_counter: 0 
            }),
            //pathology: None
        }
    }
    pub fn new_pathological( pathology: Box<Fn(String)> ) -> ContextManager {
        unimplemented!();

        // ContextManager {
        //     inner: Mutex::new(ContextManagerInner{
        //         items:          Vec::with_capacity(30),
        //         index:  Vec::with_capacity(30),
        //         vacancies:      Vec::with_capacity(30)
        //     }),
        //     pathology: Some(pathology)
        // }
    }

    /// Returns the number of subjects in the `ContextManager` including placeholders.
    pub fn subject_count(&self) -> usize {
        self.inner.lock().unwrap().index.len()
    }
    /// Returns the number of subject heads in the `ContextManager`
    pub fn subject_head_count(&self) -> usize {
        self.inner.lock().unwrap().items.iter().filter(|i| {
            if let &&Some(ref item) = i {
                if let Some(_) = item.head{
                    return true;
                }
            }
            false
        }).count()
    }
    pub fn vacancies(&self) -> usize {
        self.inner.lock().unwrap().vacancies.len()
    }

    /// Returns true if the `ContextManager` contains no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().items.is_empty()
    }

    pub fn subject_ids(&self) -> Vec<SubjectId> {
        self.inner.lock().unwrap().index.iter().map(|i| i.0 ).collect()
    }

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
    
    fn get_head_and_generation(&mut self, subject_id: SubjectId) -> (Option<MemoRefHead>, usize) {
        let inner = self.inner.lock().unwrap();
        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(item @ ContextItem{ head: Some(h), .. })) => {
                        (Some( h.clone() ), item.edit_counter)
                    },
                    None => (None,0)
                }
            }
            None => (None,0)
        }
    }
    /// For a given SubjectId, apply a MemoRefHead to the one stored in the ContextManager, but only if a MemoRefHead was already present.
    /// Return Some(applied head) or None if none was present for the provided SubjectId
    //pub fn conditional_apply_head (&self, subject_id: SubjectId, apply_head: &MemoRefHead, slab: &Slab) -> Option<MemoRefHead> {
    //}

    /// Apply the provided MemoRefHead for a given subject. Project relation references and add placeholders as needed to the context
    pub fn apply_head (&self, subject_id: SubjectId, apply_head: &MemoRefHead, slab: &Slab) -> MemoRefHead {


        // Notes on future concurrency upgrades:
        // Most likely, will want to decompose MemoRefHead here such that we can add the new memos to the set first, then remove the superseded memorefs after that.
        // Provided that the memoref add operation completes before the memoref remove operation, this should be concurrency safe (I think) so long as (non A,B,A) addition to and removal from the set is idempotent.
        // The A/B/A scenario is fine because it doesn't hurt correctness to have too much in the context, only too little.
        // Question is: how do atomics fit into this?
        //
        // Going with the low road for now:

        loop {
            let (maybe_head, edit_counter) = self.get_head_and_generation(subject_id);
            // Can't span the fetch/apply/set with a lock, due to the potential for deadlock. Therefore, employing a quick hack:
            // Any edit that is applied after this edit counter is gotten will trigger a do-over

            let head = match maybe_head {
                Some(head) => {
                    // IMPORTANT! no locks may be held here.
                    // happens-before determination may require remote memo retrieval, which is a blocking operation.
                    head.apply(apply_head, slab); 
                    head
                }
                None => {
                    apply_head.clone()
                }
            };

            // IMPORTANT! no locks may be held here.
            // projection may require memo retrieval, which is a blocking operation.
            let all_relation_links_including_empties = head.project_all_relation_links_including_empties(slab);

            {
                // Ok, no projection or happens-before determination after this
                let inner   = self.inner.lock().unwrap();
                let item_id = inner.assert_item(subject_id);

                if let Some(ref item) = inner.items[item_id] {
                    if item.edit_counter != edit_counter {
                        // Something has changed. Time for a do-over.
                        continue;
                    }

                    // TODO: Iterate only the relations that changed between the old head and the apply head

                    // For now, we are assuming that all slots are present:
                    for link in all_relation_links_including_empties {
                        if let Some(rel_item_id) = item.relations[link.slot_id as usize] {
                            // Existing relation
                            let mut rel_item = inner.items[rel_item_id];


                            // LEFT OFF HERE. Next steps:
                            // X 1. Update project_all_relation_links_including_empties to provide the heads for each relation
                            // 2. If the relation is present in the context manager, and the projected relation head descends that AND all other referents do the same
                            // 3. remove the relation head from the context manager

                            // Gut check: Is it really safe to remove a relation head if the above criteria is met?
                            // I think so, because any index updates would also be in the context, and we would take no action until all referents
                            // descended the relation. It's silly to worry about this effective referents which *aren't* in the context, because
                            // we don't guarantee anything OTHER than context-based projection

                            // QUESTION: should we return here?
                            if Some(subject_id) != link.subject_id {
                                // OK, so we're unlinking from this
                                inner.decrement_item(rel_item);
                                item.relations[link.slot_id] = None;
                            }

                            // 
                            if let Some(ref new_rel_head) = link.head {
                                // >>>> PROBLEM 1 <<<< Needs to be outside of a lock
                                // >>>> PROBLEM 2 <<<< Needs be execute for new relationships, not just existing
                                if let Some(existing_rel_head) rel_item.head {
                                    if new_rel_head.descends(existing_rel_head) {
                                        inner.increment_descendents(rel_item)
                                    }
                                }
                            }
                        }else{
                            // PROBLEM! needs to be run 
                            // NON-existing relation
                            let item_id = inner.assert_item(link.subject_id);
                            let mut rel_item = inner.items[rel_item_id];
                            inner.increment_item(&*rel_item);
                            rel_item.relations[link.slot_id] = Some(item_id);
                        }
                    }

                    return head;
                }else{
                    // shouldn't ever get here
                    panic!("sanity error - missing item");
                }
            }
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
        //             self.set_subject_head( subject_head.subject_id, relation_links , new_head );

        //         }
        //     }
        // }
    }
    pub fn add_test_subject(&self, subject_id: SubjectId, maybe_relation: Option<MemoRefHead>, slab: &Slab) -> MemoRefHead {
        let rssh = if let Some(rel_head) = maybe_relation {
            RelationSlotSubjectHead::single(0, rel_head.first_subject_id().expect("subject_id not found in relation head"), rel_head.clone())
        }else{
            RelationSlotSubjectHead::empty()
        };
        let head = slab.new_memo_basic_noparent(Some(subject_id), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();

        self.apply_head(subject_id, &head, slab)
    }
}

impl ContextManagerInner {
    /// Fetch item id for a subject if present
    pub fn get_item_id_for_subject(&self, subject_id: SubjectId ) -> Option<ItemId>{
        match self.index.binary_search_by(|x| x.0.cmp(&subject_id)){
            Ok(i)  => Some(self.index[i].1),
            Err(_) => None
        }
    }
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
    fn increment_item &mut self, item: ContextItem){
        unimplemented!();
    }
    fn decrement_item(&mut self, item: ContextItem) {
        unimplemented!();
    }
    fn increment_descendents (&mut self, item: ContextItem){
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
        SubjectHeadIter{
            manager: manager,
            edit_counter: !0,
            items: Vec::new()
        }
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

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use {Network, Slab};
    use slab::{MemoBody, RelationSlotSubjectHead};
    use super::ContextManager;

    #[test]
    fn context_manager_basic() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        let head1 = slab.new_memo_basic_noparent(Some(1),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::empty(),
                                     })
            .to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        let head2 = slab.new_memo_basic_noparent(Some(2),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::single(0, 1, head1),
                                     })
            .to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        let head3 = slab.new_memo_basic_noparent(Some(3),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::single(0, 2, head2),
                                     })
            .to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        let head4 = slab.new_memo_basic_noparent(Some(4),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::single(0, 3, head3),
                                     })
            .to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4);

        let mut iter = manager.subject_head_iter();
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(4, iter.next().expect("iter result 4 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }

    #[test]
    fn context_manager_dual_indegree_zero() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4);


        // 2[0] -> 1
        // 4[0] -> 3
        let mut iter = manager.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert_eq!(4, iter.next().expect("iter result 4 should be present").subject_id);
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    #[test]
    fn context_manager_repoint_relation() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4.clone());

        // Repoint Subject 2 slot 0 to subject 4
        let head2_b = slab.new_memo_basic(Some(2), head2, MemoBody::Relation(RelationSlotSubjectHead::single(0,4,head4) )).to_head();
        manager.set_subject_head(4, head2_b.project_all_relation_links(&slab), head2_b);


        // 2[0] -> 1
        // 4[0] -> 3
        // Then:
        // 2[0] -> 4
        
        let mut iter = manager.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert_eq!(4, iter.next().expect("iter result 4 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    #[test]
    fn context_manager_remove() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to Subject 2
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 2, head2.clone()) }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        manager.remove_head(2);
        
        let mut iter = manager.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    #[test]
    fn context_manager_add_remove_cycle() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        manager.remove_head(1);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 1);

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        manager.remove_head(2);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 1);
        manager.remove_head(3);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4);

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        manager.remove_head(4);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        let mut iter = manager.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert!(iter.next().is_none(), "iter should have ended");
    }

    #[test]
    fn context_manager_contention() {

        use std::thread;
        use std::sync::{Arc,Mutex};

        let net = Network::create_new_system();
        let slab = Slab::new(&net);

        let interloper = Arc::new(Mutex::new(1));

        let mut manager = ContextManager::new_pathological(Box::new(|caller|{
            if caller == "pre_increment".to_string() {
                interloper.lock().unwrap();
            }
        }));


        let head1 = manager.add_test_subject(1, None,        &slab);    // Subject 1 is pointing to nooobody

        let lock = interloper.lock().unwrap();
        let t1 = thread::spawn(|| {
            // should block at the first pre_increment
            let head2 = manager.add_test_subject(2, Some(head1), &slab);    // Subject 2 slot 0 is pointing to Subject 1
            let head3 = manager.add_test_subject(3, Some(head2), &slab);    // Subject 3 slot 0 is pointing to Subject 2
        });

        manager.remove_head(1);
        drop(lock);

        t1.join();

        assert_eq!(manager.contains_subject(1),      true  );
        assert_eq!(manager.contains_subject_head(1), false );
        assert_eq!(manager.contains_subject_head(2), true  );
        assert_eq!(manager.contains_subject_head(3), true  );


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        
        let mut iter = manager.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 1 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    
}
