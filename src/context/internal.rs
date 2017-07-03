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
        self.stash.apply_head(&self.slab, subject_id, apply_head, &self.slab);
    }
}