// attic is not a module

/*

//TODO: update OtherSlab to use MemoPeer?
#[derive(Debug)]
pub enum MemoOrigin<'a>{
    SameSlab,
    OtherSlab(&'a SlabRef, MemoPeeringStatus)
    // TODO: consider bifurcation into OtherSlabTrusted, OtherSlabUntrusted
    //       in cases where we want to reduce computational complexity by foregoing verification
}

*/

// TODO: convert this to reconstitute_memos ( plural )
/*
    pub fn put_memos(&self, memo_origin: &MemoOrigin, mut memos: Vec<Memo> ) -> Vec<MemoRef>{

        // TODO: Evaluate more efficient ways to group these memos by subject
        let mut subject_updates : HashMap<SubjectId, MemoRefHead> = HashMap::new();
        let mut memorefs = Vec::with_capacity( memos.len() );
        let mut pre_existing = 0u64;

        for memo in memos.drain(..){
            let (memoref, pre_existed) = self.memoref_from_memo_and_origin( memo, memo_origin );
            if pre_existed { pre_existing += 1 }

            self.handle_memoref( memo_origin, &memoref ); // located in memohandling.rs

            if let Some(subject_id) = memoref.subject_id {
                let mut head = subject_updates.entry( subject_id ).or_insert( MemoRefHead::None );
                head.apply_memoref(&memoref, self);
            }

            memorefs.push(memoref);
        }

        {
            let mut counters = self.counters.write().unwrap();
            counters.memos_received += memorefs.len() as u64;
            counters.memos_redundantly_received += pre_existing;
        }

        for (subject_id,head) in subject_updates {
            self.dispatch_subject_head(subject_id, &head);
        }

        memorefs
    }
*/
