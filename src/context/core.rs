use super::*;
use slab::{RelationSlotId,RelationLink};
use subject::{Subject,SubjectCore,SubjectId,SubjectType};
use memorefhead::*;
use index::IndexFixed;
use error::*;

use std::sync::{Arc,Mutex,RwLock};
use std::collections::HashMap;
use std::ops::Deref;

type ItemId = usize;

struct ContextItem {
    subject_id:   SubjectId,
    refcount:     usize,
    head:         Option<MemoRefHead>,
    relations:    Vec<Option<ItemId>>,
    edit_counter: usize,
}

#[derive(Clone)]
pub struct ContextCore(Arc<ContextCoreInner>);

impl Deref for ContextCore {
    type Target = ContextCoreInner;
    fn deref(&self) -> &ContextCoreInner {
        &*self.0
    }
}

/// Performs topological sorting.
pub struct ContextCoreInner {
    pub slab: Slab,
    pub root_index: RwLock<Option<IndexFixed>>,
    heads:      Mutex<ContextSubjectHeads>,
    //pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}

#[derive(Default)]
struct ContextSubjectHeads{
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
            relations: Vec::new(),
            edit_counter: 0,
            refcount: 0
        }
    }
}



impl ContextCore {
    pub fn new (slab: &Slab) -> Self {
        let new_self = ContextCore(
            Arc::new(
                ContextCoreInner{
                    slab: slab.clone(),
                    root_index: RwLock::new(None),
                    heads: Default::default()
                }
            )
        );

        // Typically subjects, and the indexes that use them, have a hard link to their originating
        // contexts. This is useful because we want to make sure the context (and associated slab)
        // stick around until we're done with them

        // The root index is a bit of a special case however, because the context needs to have a hard link to it,
        // as it must use the index directly. Therefore I need to make sure it doesn't have a hard link back to me.
        // This shouldn't be a problem, because the index is private, and not subject to direct use, so the context
        // should outlive it.

        let seed = slab.get_root_index_seed().expect("Uninitialized slab");

        let index = IndexFixed::new_from_memorefhead(&new_self, 5, seed);

        *new_self.root_index.write().unwrap() = Some(index);

        new_self
    }

    pub fn insert_into_root_index(&self, subject_id: SubjectId, subject: &SubjectCore) {
        if let Some(ref index) = *self.root_index.write().unwrap() {
            index.insert(self,subject_id, subject);
        } else {
            panic!("no root index")
        }
    }

    /// Retrieve a subject for a known MemoRefHead – ususally used for relationship traversal.
    /// Any relevant context will also be applied when reconstituting the relevant subject to ensure that our consistency model invariants are met
    pub fn get_subject_with_head(&self,
                                 subject_id: SubjectId,
                                 mut head: MemoRefHead)
                                 -> Result<SubjectCore, RetrieveError> {
        // println!("# Context.get_subject_with_head({},{:?})", subject_id, head.memo_ids() );

        if head.len() == 0 {
            return Err(RetrieveError::InvalidMemoRefHead);
        }

        let maybe_head = {
            // Don't want to hold the lock while calling head.apply, as it could request a memo from a remote slab, and we'd deadlock
            if let Some(ref head) = self.get_head(subject_id) {
                Some((*head).clone())
            } else {
                None
            }
        };

        if let Some(relevant_context_head) = maybe_head {
            // println!("# \\ Relevant context head is ({:?})", relevant_context_head.memo_ids() );
            head.apply(&relevant_context_head, &self.slab);

        } else {
            // println!("# \\ No relevant head found in context");
        }

        match self.get_subject_if_resident(subject_id) {
            Some(ref mut subject) => {
                subject.apply_head(&head);
                return Ok(subject.clone());
            }
            None => {}
        }

        // NOTE: Subject::reconstitute calls back to Context.subscribe_subject()
        //       so we need to release the mutex prior to this
        let subject = SubjectCore::reconstitute(&self, head);
        return Ok(subject);

    }

        /// Subscribes a resident subject struct to relevant updates from this context
    /// Used by the subject constructor
    pub fn subscribe_subject(&self, subject: &SubjectCore) {
        // println!("Context.subscribe_subject({})", subject.id );
        // {
        //     self.subjects.write().unwrap().insert(subject.id, subject.weak());
        // }

        // TODO: determine if we want to use channels, or futures streams, or what.
        //       giving out Arcs to the context doesn't seem like the way to go
        //self.slab.subscribe_subject(subject.id, self);
    }
    /// Unsubscribes the subject from further updates. Used by Subject.drop
    /// ( Temporarily defeated due to deadlocks. TODO )
    pub fn unsubscribe_subject(&self, subject_id: SubjectId) {
        // println!("# Context.unsubscribe_subject({})", subject_id);
        // let _ = subject_id;
        //self.subjects.write().unwrap().remove(&subject_id);

        // BUG/TODO: Temporarily disabled unsubscription
        // 1. Because it was causing deadlocks on the context AND slab mutexes
        // when the thread in the test case happened to drop the subject
        // when we were busy doing apply_subject_head, which locks context,
        // and is called by slab – so clearly this is untenable
        // 2. It was always sort of a hack that the subject was managing subscriptions
        // in this way anyways. Lets put together a more final version of the subscriptions
        // before we bother with fixing unsubscription
        //
        // {
        // let mut shared = self.inner.shared.lock().unwrap();
        // shared.subjects.remove( &subject_id );
        // }
        //
        // self.inner.slab.unsubscribe_subject(subject_id, self);
        // println!("# Context.unsubscribe_subject({}) - FINISHED", subject_id);
        //

    }

    /// Retrieves a subject by ID from this context only if it is currently resedent
    fn get_subject_if_resident(&self, subject_id: SubjectId) -> Option<Subject> {

        unimplemented!()
        // if let Some(weaksub) = self.subjects.read().unwrap().get(&subject_id) {
        //     if let Some(subject) = weaksub.upgrade() {
        //         // NOTE: In theory we shouldn't need to apply the current context
        //         //      to this subject, as it shouldddd have already happened
        //         return Some(subject);
        //     }
        // }

        // None
    }
    /// Called by the Slab whenever memos matching one of our subscriptions comes in, or by the Subject when an edit is made
    pub fn apply_subject_head(&self, subject_id: SubjectId, apply_head: &MemoRefHead) {
        // println!("Context.apply_subject_head({}, {:?}) ", subject_id, head.memo_ids() );

        //self.manager.apply_head(subject_id, apply_head, &self.slab);


    }
    /// For a given SubjectId, apply a MemoRefHead to the one stored in the ContextManager, but only if a MemoRefHead was already present.
    /// Return Some(applied head) or None if none was present for the provided SubjectId
    //pub fn conditional_apply_head (&self, subject_id: SubjectId, apply_head: &MemoRefHead, slab: &Slab) -> Option<MemoRefHead> {
    //}

    /// Apply the provided MemoRefHead for a given subject. Project relation references and add placeholders as needed to the context
    pub fn apply_head (&self, subject_id: SubjectId, apply_head: &MemoRefHead, notify_subject: bool) -> MemoRefHead {

        // LEFT OFF HERERERERERERERERERERER
        // Think more aboot this
        if apply_head.is_root_index() {
            // lock
            //     get existing head and edit number / generation
            // unlock
            // project child relations
            // iterate over any resident children
            // expunge them if they descend or equal the references
        }else{
            // iterate over resident parent relation(s?)
            // NoOp if they descend or are equal
        }

        if notify_subject {
            if let Some(ref subject) = self.get_subject_if_resident(subject_id) {
                subject.apply_head(apply_head)
            }
        }
    }
    fn apply_head_parts_bin_delete_me () {
        //TODO: refactor the below into other functions
        unimplemented!();

        // Notes on future concurrency upgrades:
        // Most likely, will want to decompose MemoRefHead here such that we can add the new memos to the set first, then remove the superseded memorefs after that.
        // Provided that the memoref add operation completes before the memoref remove operation, this should be concurrency safe (I think) so long as (non A,B,A) addition to and removal from the set is idempotent.
        // The A/B/A scenario is fine because it doesn't hurt correctness to have too much in the context, only too little.
        // Question is: how do atomics fit into this?
        //
        // Going with the low road for now:
        // * get the resident head (if any) and generation for the subject id in question
        // * project all head links
        // * 

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

            // when inserting, 



            // IMPORTANT! no locks may be held here.
            // projection may require memo retrieval, which is a blocking operation.
            let all_head_links_including_empties = head.project_all_head_links_including_empties(slab);

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
                    for link in all_head_links_including_empties {
                        if let Some(rel_item_id) = item.relations[link.slot_id as usize] {
                            // Existing relation
                            let mut rel_item = inner.items[rel_item_id].as_ref().expect("inner.items sanity error");


                            // LEFT OFF HERE. Next steps:
                            // X 1. Update project_all_relation_links_including_empties to provide the heads for each relation
                            // 2. If the relation is present in the context manager, and the projected relation head descends that AND all other referents do the same
                            // 3. remove the relation head from the context manager

                            // Automatic pruning of subject heads was intended to produce a context which was always equal to or descendant of the pre-pruned context.
                            // Pruning of subject heads for which N of N referents are desdendant makes less sense given that the revised context would *not*
                            // be functionally equivalent to its predecessor, instead requiring a root index traversal to inforce its invariants.

                            // >> This seems problematic.
                            // Assuming that the in the most superficial sense

                            // QUESTION: should we return here?
                            if Some(subject_id) != link.subject_id {
                                // OK, so we're unlinking from this
                                inner.decrement_item(rel_item);
                                item.relations[link.slot_id as usize] = None;
                            }

                            // 
                            if let Some(ref new_rel_head) = link.head {
                                // >>>> PROBLEM 1 <<<< Needs to be outside of a lock
                                // >>>> PROBLEM 2 <<<< Needs be execute for new relationships, not just existing
                                if let Some(ref existing_rel_head) = rel_item.head {
                                    if new_rel_head.descends(existing_rel_head, slab) {

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

    pub fn add_test_subject(&self, subject_id: SubjectId, maybe_relation: Option<MemoRefHead>, slab: &Slab) -> MemoRefHead {
        let rssh = if let Some(rel_head) = maybe_relation {
            RelationSlotSubjectHead::single(0, rel_head.first_subject_id().expect("subject_id not found in relation head"), rel_head.clone())
        }else{
            RelationSlotSubjectHead::empty()
        };
        let memobody = MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty(), t: SubjectType::Record };
        let head = slab.new_memo_basic_noparent(Some(subject_id), memobody).to_head();

        self.apply_head(subject_id, &head, false)
    }
}


impl Drop for ContextCore {
    fn drop(&mut self) {
        // println!("# ContextShared.drop");
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use {Network, Slab};
    use slab::{MemoBody, RelationSlotSubjectHead};
    use super::ContextCore;

    #[test]
    fn context_core_basic() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut core = ContextCore::new(&slab);

        // 4 -> 3 -> 2 -> 1
        let head1 = core.add_test_subject(1, None, &slab        );
        let head2 = core.add_test_subject(2, Some(head1), &slab );
        let head3 = core.add_test_subject(3, Some(head2), &slab );
        let head4 = core.add_test_subject(4, Some(head3), &slab );


        let mut iter = core.subject_head_iter();
        assert!(iter.get_subject_ids() == [1,2,3,4], "Valid sequence");
    }

    #[test]
    fn context_manager_dual_indegree_zero() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut core = ContextCore::new(&slab);

        // 2 -> 1, 4 -> 3
        let head1 = core.add_test_subject(1, None, &slab        );
        let head2 = core.add_test_subject(2, Some(head1), &slab );
        let head3 = core.add_test_subject(3, None,        &slab );
        let head4 = core.add_test_subject(4, Some(head3), &slab );

        let mut iter = core.subject_head_iter();
        assert!(iter.get_subject_ids() == [1,3,2,4], "Valid sequence");
    }
    #[test]
    fn context_manager_repoint_relation() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut core = ContextCore::new(&slab);

        // 2 -> 1, 4 -> 3
        // Then:
        // 2 -> 4
        
        let head1 = core.add_test_subject(1, None, &slab        );
        let head2 = core.add_test_subject(2, Some(head1), &slab );
        let head3 = core.add_test_subject(3, None,        &slab );
        let head4 = core.add_test_subject(4, Some(head3), &slab );

        // Repoint Subject 2 slot 0 to subject 4
        let head2_b = slab.new_memo_basic(Some(2), head2, MemoBody::Relation(RelationSlotSubjectHead::single(0,4,head4) )).to_head();
        core.apply_head(4, &head2_b, &slab);

        let mut iter = core.subject_head_iter();
        assert!(iter.get_subject_ids() == [1,4,3,2], "Valid sequence");
    }
    #[test]
    fn context_manager_remove() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut core = ContextCore::new(&slab);

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        core.apply_head(1, head1.project_all_relation_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        core.apply_head(2, head2.project_all_relation_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to Subject 2
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 2, head2.clone()) }).to_head();
        core.apply_head(3, head3.project_all_relation_links(&slab), head3.clone());


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        core.remove_head(2);
        
        let mut iter = core.subject_head_iter();
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
        let mut core = ContextCore::new(&slab);

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        core.apply_head(1, head1.project_all_relation_links(&slab), head1.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        core.remove_head(1);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 1);

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        core.apply_head(2, head2.project_all_relation_links(&slab), head2.clone());

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        core.remove_head(2);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        core.apply_head(3, head3.project_all_relation_links(&slab), head3.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 1);
        core.remove_head(3);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        core.apply_head(4, head4.project_all_relation_links(&slab), head4);

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        core.remove_head(4);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        let mut iter = core.subject_head_iter();
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


        let head1 = core.add_test_subject(1, None,        &slab);    // Subject 1 is pointing to nooobody

        let lock = interloper.lock().unwrap();
        let t1 = thread::spawn(|| {
            // should block at the first pre_increment
            let head2 = core.add_test_subject(2, Some(head1), &slab);    // Subject 2 slot 0 is pointing to Subject 1
            let head3 = core.add_test_subject(3, Some(head2), &slab);    // Subject 3 slot 0 is pointing to Subject 2
        });

        core.remove_head(1);
        drop(lock);

        t1.join();

        assert_eq!(manager.contains_subject(1),      true  );
        assert_eq!(manager.contains_subject_head(1), false );
        assert_eq!(manager.contains_subject_head(2), true  );
        assert_eq!(manager.contains_subject_head(3), true  );


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        
        let mut iter = core.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 1 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    
}