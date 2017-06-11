pub mod serde;
mod projection;

use slab::*;
use subject::*;
use context::*;
use error::*;

use std::mem;
use std::fmt;
use std::slice;
use std::collections::VecDeque;

// MemoRefHead is a list of MemoRefs that constitute the "head" of a given causal chain
//
// This "head" is rather like a git HEAD, insofar as it is intended to contain only the youngest
// descendents of a given causal chain. It provides mechanisms for applying memorefs, or applying
// other MemoRefHeads such that the mutated list may be pruned as appropriate given the above.


pub type RelationSlotId = u8;

#[derive(Clone, PartialEq)]
pub struct MemoRefHead (Vec<MemoRef>);

pub struct RelationLink{
    pub slot_id:    RelationSlotId,
    pub subject_id: Option<SubjectId>,
    pub head:       Option<MemoRefHead>
}

impl MemoRefHead {
    pub fn new () -> Self {
        MemoRefHead( Vec::with_capacity(5) )
    }
    pub fn new_from_vec ( vec: Vec<MemoRef> ) -> Self {
        MemoRefHead( vec )
    }
    pub fn from_memoref (memoref: MemoRef) -> Self {
        MemoRefHead( vec![memoref] )
    }
    pub fn apply_memoref(&mut self, new: &MemoRef, slab: &Slab ) -> bool {
        //println!("# MemoRefHead({:?}).apply_memoref({})", self.memo_ids(), &new.id);

        // Conditionally add the new memoref only if it descends any memorefs in the head
        // If so, any memorefs that it descends must be removed

        // Not suuuper in love with these flag names
        let mut new_is_descended = false;
        let mut new_descends  = false;

        let mut applied  = false;
        let mut replaced  = false;

        // I imagine it's more efficient to iterate in reverse, under the supposition that
        // new items are more likely to be at the end, and that's more likely to trigger
        // the cheapest case: (existing descends new)

        'existing: for i in (0..self.0.len()).rev() {
            let mut remove = false;
            {
                let ref mut existing = self.0[i];
                if existing == new {
                    return false; // we already had this

                } else if existing.descends(&new,&slab) {
                    new_is_descended = true;

                    // IMPORTANT: for the purposes of the boolean return,
                    // the new memo does not get "applied" in this case

                    // If any memo in the head already descends the newcomer,
                    // then it doesn't get applied at all punt the whole thing
                    break 'existing;

                } else if new.descends(&existing, &slab) {
                    new_descends = true;
                    applied = true; // descends

                    if replaced {
                        remove = true;
                    }else{
                        // Lets try real hard not to remove stuff in the middle of the vec
                        // But we only get to do this trick once, because we don't want to add duplicates
                        mem::replace( existing, new.clone() );
                        replaced = true;
                    }

                }
            }

            if remove {
                // because we're descending, we know the offset of the next items won't change
                self.0.remove(i);
            }
        }

        if !new_descends && !new_is_descended  {
            // if the new memoref neither descends nor is descended
            // then it must be concurrent

            self.0.push(new.clone());
            applied = true; // The memoref was "applied" to the MemoRefHead
        }

        // This memoref was applied if it was concurrent, or descends one or more previous memos

        if applied {
            //println!("# \t\\ Was applied - {:?}", self.memo_ids());
        }else{
            //println!("# \t\\ NOT applied - {:?}", self.memo_ids());
        }

        applied
    }
    pub fn apply_memorefs (&mut self, new_memorefs: &Vec<MemoRef>, slab: &Slab) {
        for new in new_memorefs.iter(){
            self.apply_memoref(new, slab);
        }
    }
    pub fn apply (&mut self, other: &MemoRefHead, slab: &Slab){
        for new in other.iter(){
            self.apply_memoref( new, slab );
        }
    }
    /// Test to see if this MemoRefHead fully descends another
    /// If there is any hint of causal concurrency, then this will return false
    pub fn descends (&self, other: &MemoRefHead, slab: &Slab) -> bool{
        if self.len() == 0 || other.len() == 0 {
            return false // searching for positive descendency, not merely non-ascendency
        }

        // there's probably a more efficient way to do this than iterating over the cartesian product
        // we can get away with it for now though I think
        // TODO: revisit when beacons are implemented
        for memoref in self.iter(){
            for other_memoref in other.iter(){
                if !memoref.descends(other_memoref, slab) {
                    return false;
                }
            }
        }

        return true;
    }
    pub fn memo_ids (&self) -> Vec<MemoId> {
        self.0.iter().map(|m| m.id).collect()
    }
    pub fn first_subject_id (&self) -> Option<SubjectId> {
        if let Some(memoref) = self.iter().next() {
            // TODO: Could stand to be much more robust here
            memoref.subject_id
        }else{
            None
        }
    }
    pub fn to_vec (&self) -> Vec<MemoRef> {
        self.0.clone()
    }
    pub fn to_vecdeque (&self) -> VecDeque<MemoRef> {
        VecDeque::from(self.0.clone())
    }
    pub fn len (&self) -> usize {
        self.0.len()
    }
    pub fn iter (&self) -> slice::Iter<MemoRef> {
        self.0.iter()
    }
    pub fn causal_memo_iter(&self, slab: &Slab ) -> CausalMemoIter {
        CausalMemoIter::from_head( &self, slab )
    }
    pub fn is_fully_materialized(&self, slab: &Slab ) -> bool {
        // TODO: consider doing as-you-go distance counting to the nearest materialized memo for each descendent
        //       as part of the list management. That way we won't have to incur the below computational effort.

        for memoref in self.iter(){
            if let Ok(memo) = memoref.get_memo(slab) {
                match memo.body {
                    MemoBody::FullyMaterialized { v: _, r: _ } => {},
                    _                           => { return false }
                }
            }else{
                // TODO: do something more intelligent here
                panic!("failed to retrieve memo")
            }
        }

        true
    }
    pub fn clone_for_slab (&self, from_slabref: &SlabRef, to_slab: &Slab, include_memos: bool ) -> Self {
        assert!(from_slabref.slab_id != to_slab.id, "slab id should differ");
        MemoRefHead( self.iter().map(|mr| mr.clone_for_slab(from_slabref, to_slab, include_memos )).collect() )
    }
}

impl fmt::Debug for MemoRefHead{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {

        fmt.debug_struct("MemoRefHead")
            .field("memo_refs", &self.0 )
            //.field("memo_ids", &self.memo_ids() )
            .finish()
    }
}

pub struct CausalMemoIter {
    queue: VecDeque<MemoRef>,
    slab:  Slab
}

/*
  Plausible Memo Structure:
          /- E -> C -\
     G ->              -> B -> A
head ^    \- F -> D -/
     Desired iterator sequence: G, E, C, F, D, B, A ( Why? )
     Consider:                  [G], [E,C], [F,D], [B], [A]
     Arguably this should not be an iterator at all, but rather a recursive function
     Going with the iterator for now in the interest of simplicity
*/
impl CausalMemoIter {
    pub fn from_head ( head: &MemoRefHead, slab: &Slab) -> Self {
        //println!("# -- SubjectMemoIter.from_head({:?})", head.memo_ids() );

        CausalMemoIter {
            queue: head.to_vecdeque(),
            slab:  slab.clone()
        }
    }
}
impl Iterator for CausalMemoIter {
    type Item = Memo;

    fn next (&mut self) -> Option<Memo> {
        // iterate over head memos
        // Unnecessarly complex because we're not always dealing with MemoRefs
        // Arguably heads should be stored as Vec<MemoRef> instead of Vec<Memo>

        // TODO: Stop traversal when we come across a Keyframe memo
        if let Some(memoref) = self.queue.pop_front() {
            // this is wrong - Will result in G, E, F, C, D, B, A

            match memoref.get_memo( &self.slab ){
                Ok(memo) => {
                    self.queue.append(&mut memo.get_parent_head().to_vecdeque());
                    return Some(memo)
                },
                Err(err) => {
                    panic!("Failed to retrieve memo {} ({:?})", memoref.id, err );
                }
            }
            //TODO: memoref.get_memo needs to be able to fail
        }

        return None;
    }
}
