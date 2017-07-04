use super::*;

/// Internal interface functions
impl Context {
    pub fn insert_into_root_index(&self, subject_id: SubjectId, subject: &SubjectCore) {
        if let Some(ref index) = *self.root_index.write().unwrap() {
            index.insert(self,subject_id, subject);
        } else {
            panic!("no root index")
        }
    }

    /// Called by the Slab whenever memos matching one of our subscriptions comes in, or by the Subject when an edit is made
    pub fn apply_subject_head(&self, subject_id: SubjectId, apply_head: &MemoRefHead) {
        // println!("Context.apply_subject_head({}, {:?}) ", subject_id, head.memo_ids() );
        self.stash.apply_head(&self.slab, subject_id, apply_head);
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
}