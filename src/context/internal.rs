use super::*;

/// Internal interface functions
impl Context {
    pub (crate) fn insert_into_root_index(&self, subject_id: SubjectId, subject: &SubjectCore) {
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
    pub fn get_subject_core(&self, subject_id: SubjectId) -> Result<SubjectCore, RetrieveError> {
        match *self.root_index.read().unwrap() {
            Some(ref index) => index.get(&self, subject_id),
            None            => Err(RetrieveError::IndexNotInitialized),
        }
    }
    /// Retrieves a subject by ID from this context only if it is currently resedent
    fn get_subject_if_resident(&self, subject_id: SubjectId) -> Option<SubjectCore> {

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
    pub (crate) fn subscribe_subject(&self, subject: &SubjectCore) {
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