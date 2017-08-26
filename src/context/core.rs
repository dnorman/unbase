





impl ContextCore {

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

}


impl Drop for ContextCore {
    fn drop(&mut self) {
        // println!("# ContextShared.drop");
    }
}

