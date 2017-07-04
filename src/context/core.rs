





impl ContextCore {

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

    /// For a given SubjectId, apply a MemoRefHead to the one stored in the ContextManager, but only if a MemoRefHead was already present.
    /// Return Some(applied head) or None if none was present for the provided SubjectId
    //pub fn conditional_apply_head (&self, subject_id: SubjectId, apply_head: &MemoRefHead, slab: &Slab) -> Option<MemoRefHead> {
    //}

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