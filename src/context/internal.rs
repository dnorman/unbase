use super::*;


/// Internal interface functions
impl Context {
    pub (crate) fn insert_into_root_index(&self, subject_id: SubjectId, subject: &Subject) {
        if let Some(ref index) = *self.root_index.write().unwrap() {
            index.insert(self,subject_id, subject);
        } else {
            panic!("no root index")
        }
    }

    /// Called by the Slab whenever memos matching one of our subscriptions comes in, or by the Subject when an edit is made
    pub (crate) fn apply_head(&self, subject_id: SubjectId, apply_head: &MemoRefHead) -> MemoRefHead {
        // println!("Context.apply_subject_head({}, {:?}) ", subject_id, head.memo_ids() );
        self.stash.apply_head(&self.slab, subject_id, apply_head)
    }
    pub fn get_subject_core(&self, subject_id: SubjectId) -> Result<Subject, RetrieveError> {
        match *self.root_index.read().unwrap() {
            Some(ref index) => index.get(&self, subject_id),
            None            => Err(RetrieveError::IndexNotInitialized),
        }
    }
    /// Retrieves a subject by ID from this context only if it is currently resedent
    fn get_subject_core_if_resident(&self, subject_id: SubjectId) -> Option<Subject> {
        
        TODO:

        // Consider getting rid of Subject(Arc<SubjectInner>) in favor of renaming SubjectInner to Subject
        // and having Subject be an observable which injects a channel into the context


        //NEXT - Figure out how observables work, and decide if
        // we should be storing those, or storing weak refs to subject cores
        // Should there only ever be one copy of a subject core resident per process?
        // This seems wrongggg, and non concurrency-friendly

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
    /// Retrieve a subject for a known MemoRefHead – ususally used for relationship traversal.
    /// Any relevant context will also be applied when reconstituting the relevant subject to ensure that our consistency model invariants are met
    pub fn get_subject_core_with_head(&self,  mut head: MemoRefHead)  -> Result<Subject, RetrieveError> {
        // println!("# Context.get_subject_with_head({},{:?})", subject_id, head.memo_ids() );

        if head.len() == 0 {
            return Err(RetrieveError::InvalidMemoRefHead);
        }

        let maybe_subject_id = head.subject_id();
        let maybe_head = {
            // Don't want to hold the lock while calling head.apply, as it could request a memo from a remote slab, and we'd deadlock
            if let Some(subject_id) = maybe_subject_id {
                if let Some(ref head) = self.stash.get_head(subject_id) {
                    Some((*head).clone())
                } else {
                    None
                }
            }else{
                None
            }
        };

        if let Some(relevant_context_head) = maybe_head {
            // println!("# \\ Relevant context head is ({:?})", relevant_context_head.memo_ids() );
            head.apply(&relevant_context_head, &self.slab);

        } else {
            // println!("# \\ No relevant head found in context");
        }

        if let Some(subject_id) = maybe_subject_id {
            match self.get_subject_core_if_resident(subject_id) {
                Some(ref mut subject) => {
                    subject.apply_head(self, &head);
                    return Ok(subject.clone());
                }
                None => {}
            };
        };

        // NOTE: Subject::reconstitute calls back to Context.subscribe_subject()
        //       so we need to release the mutex prior to this
        let subject = Subject
::reconstitute(&self, head);
        return Ok(subject);

    }
    pub (crate) fn subscribe_subject(&self, subject: &Subject) {
        unimplemented!()
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
    pub (crate) fn unsubscribe_subject(&self, subject_id: SubjectId) {
        unimplemented!()
    }
}