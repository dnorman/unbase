pub mod serde;
mod projection;

use slab::{Slab,SlabRef,Memo,MemoId,MemoBody,MemoRef,EdgeLink,RelationSlotId};
use subject::*;
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

#[derive(Clone, PartialEq)]
pub enum MemoRefHead {
    Null,
    Subject{
        subject_id: SubjectId,
        head:       Vec<MemoRef>
    },
    Anonymous{
        head:       Vec<MemoRef>
    }
}

impl MemoRefHead {
    // pub fn new_from_vec ( vec: Vec<MemoRef>, slab: &Slab ) -> Self {

    //     unimplemented!()
    //     // for memoref in vec.iter(){
    //     //     if let Ok(memo) = memoref.get_memo(slab) {
    //     //         match memo.body {
    //     //             MemoBody::FullyMaterialized { t, .. } => {
    //     //                 return t;
    //     //             }
    //     //         }
    //     //     }else{
    //     //         // TODO: do something more intelligent here
    //     //         panic!("failed to retrieve memo")
    //     //     }
    //     // }

    //     // panic!("no FullyMaterialized memobody found")

    //     // if vec.len() > 0 {
    //     //     MemoRefHead::Some{
    //     //         subject_id: vec[0].get_subject_id(),
    //     //         stype:      
    //     //     }
    //     // }else{
    //     //     MemoRefHead::Null
    //     // }
    // }
    pub fn apply_memoref(&mut self, new: &MemoRef, slab: &Slab ) -> Result<bool,WriteError> {
        //println!("# MemoRefHead({:?}).apply_memoref({})", self.memo_ids(), &new.id);

        // Conditionally add the new memoref only if it descends any memorefs in the head
        // If so, any memorefs that it descends must be removed
        let head = match *self {
            MemoRefHead::Null => {
                if let Some(subject_id) = new.subject_id {
                    *self = MemoRefHead::Subject{
                        head: vec![new.clone()],
                        subject_id
                    };
                }else{
                    *self = MemoRefHead::Anonymous{ head: vec![new.clone()] };
                }

                return Ok(true);
            },
            MemoRefHead::Anonymous{ ref mut head } => {
                head
            },
            MemoRefHead::Subject{ ref mut head, ..} => {
                head
            }
        };

        // Not suuuper in love with these flag names
        let mut new_is_descended = false;
        let mut new_descends  = false;

        let mut applied  = false;
        let mut replaced  = false;

        // I imagine it's more efficient to iterate in reverse, under the supposition that
        // new items are more likely to be at the end, and that's more likely to trigger
        // the cheapest case: (existing descends new)

        'existing: for i in (0..head.len()).rev() {
            let mut remove = false;
            {
                let ref mut existing = head[i];
                if existing == new {
                    return Ok(false); // we already had this

                } else if existing.descends(&new,&slab)? {
                    new_is_descended = true;

                    // IMPORTANT: for the purposes of the boolean return,
                    // the new memo does not get "applied" in this case

                    // If any memo in the head already descends the newcomer,
                    // then it doesn't get applied at all punt the whole thing
                    break 'existing;

                } else if new.descends(&existing, &slab)? {
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
                head.remove(i);
            }
        }

        if !new_descends && !new_is_descended  {
            // if the new memoref neither descends nor is descended
            // then it must be concurrent

            head.push(new.clone());
            applied = true; // The memoref was "applied" to the MemoRefHead
        }

        // This memoref was applied if it was concurrent, or descends one or more previous memos

        if applied {
            //println!("# \t\\ Was applied - {:?}", self.memo_ids());
        }else{
            //println!("# \t\\ NOT applied - {:?}", self.memo_ids());
        }

       Ok(applied)
    }
    pub fn apply_memorefs (&mut self, new_memorefs: &Vec<MemoRef>, slab: &Slab) -> Result<(),WriteError> {
        for new in new_memorefs.iter(){
            self.apply_memoref(new, slab)?;
        }
        Ok(())
    }
    pub fn apply (&mut self, other: &MemoRefHead, slab: &Slab) -> Result<bool,WriteError> {
        let mut applied = false;

        for new in other.iter(){
            if self.apply_memoref( new, slab )? {
                applied = true;
            }
        }

        Ok(applied)
    }

    /// Test to see if this MemoRefHead fully descends another
    /// If there is any hint of causal concurrency, then this will return false
    pub fn descends_or_contains (&self, other: &MemoRefHead, slab: &Slab) -> Result<bool,RetrieveError> {

        // there's probably a more efficient way to do this than iterating over the cartesian product
        // we can get away with it for now though I think
        // TODO: revisit when beacons are implemented
        match *self {
            MemoRefHead::Null             => Ok(false),
            MemoRefHead::Subject{ ref head, .. } | MemoRefHead::Anonymous{ ref head, .. } => {
                match *other {
                    MemoRefHead::Null             => Ok(false),
                    MemoRefHead::Subject{ head: ref other_head, .. } | MemoRefHead::Anonymous{ head: ref other_head, .. } => {
                        if head.len() == 0 || other_head.len() == 0 {
                            return Ok(false) // searching for positive descendency, not merely non-ascendency
                        }
                        for memoref in head.iter(){
                            'other: for other_memoref in other_head.iter(){
                                if memoref == other_memoref {
                                    //
                                } else if !memoref.descends(other_memoref, slab)? {
                                    return Ok(false);
                                }
                            }
                        }

                        Ok(true)
                    }
                }
            }
        }
    }
    pub fn memo_ids (&self) -> Vec<MemoId> {
        match *self {
            MemoRefHead::Null => Vec::new(),
            MemoRefHead::Subject{ ref head, .. } | MemoRefHead::Anonymous{ ref head, .. } => head.iter().map(|m| m.id).collect()
        }
    }
    pub fn subject_id (&self) -> Option<SubjectId> {
        match *self {
            MemoRefHead::Null | MemoRefHead::Anonymous{..} => None,
            MemoRefHead::Subject{ subject_id, .. }     => Some(subject_id)
        }
    }
    pub fn is_some (&self) -> bool {
        match *self {
            MemoRefHead::Null => false,
            _                 => true
        }
    }
    pub fn to_vec (&self) -> Vec<MemoRef> {
        match *self {
            MemoRefHead::Null => vec![],
            MemoRefHead::Anonymous { ref head, .. } => head.clone(),
            MemoRefHead::Subject{  ref head, .. }   => head.clone()
        }
    }
    pub fn to_vecdeque (&self) -> VecDeque<MemoRef> {
        match *self {
            MemoRefHead::Null       => VecDeque::new(),
            MemoRefHead::Anonymous { ref head, .. } => VecDeque::from(head.clone()),
            MemoRefHead::Subject{  ref head, .. }   => VecDeque::from(head.clone())
        }
    }
    pub fn len (&self) -> usize {
        match *self {
            MemoRefHead::Null       =>  0,
            MemoRefHead::Anonymous { ref head, .. } => head.len(),
            MemoRefHead::Subject{  ref head, .. }   => head.len()
        }
    }
    pub fn iter (&self) -> slice::Iter<MemoRef> {

        // This feels pretty stupid. Probably means something is wrong with the factorization of MRH
        static EMPTY : &'static [MemoRef] = &[];

        match *self {
            MemoRefHead::Null                    => EMPTY.iter(), // HACK
            MemoRefHead::Anonymous{ ref head }   => head.iter(),
            MemoRefHead::Subject{ ref head, .. } => head.iter()
        }
    }
    pub fn causal_memo_iter(&self, slab: &Slab ) -> CausalMemoIter {
        CausalMemoIter::from_head( &self, slab )
    }
    pub fn is_fully_materialized(&self, slab: &Slab ) -> bool {
        // TODO: consider doing as-you-go distance counting to the nearest materialized memo for each descendent
        //       as part of the list management. That way we won't have to incur the below computational effort.

        let head = match *self {
            MemoRefHead::Null       => {
                return true;
            },
            MemoRefHead::Anonymous { ref head, .. } => head,
            MemoRefHead::Subject{  ref head, .. } => head
        };
        
        for memoref in head.iter(){
            let memo = memoref.get_memo(slab).unwrap();

            if let MemoBody::FullyMaterialized { .. } = memo.body {
                //
            }else{
                return false
            }
        }

        true
    }
    pub fn clone_for_slab (&self, from_slabref: &SlabRef, to_slab: &Slab, include_memos: bool ) -> Self {
        assert!(from_slabref.slab_id != to_slab.id, "slab id should differ");
        match *self {
            MemoRefHead::Null                    => MemoRefHead::Null,
            MemoRefHead::Anonymous { ref head }  => MemoRefHead::Anonymous{
                head: head.iter().map(|mr| mr.clone_for_slab(from_slabref, to_slab, include_memos )).collect()
            },
            MemoRefHead::Subject{ subject_id, ref head } => MemoRefHead::Subject {
                subject_id: subject_id,
                head:       head.iter().map(|mr| mr.clone_for_slab(from_slabref, to_slab, include_memos )).collect()
            }
        }
    }
}

impl fmt::Debug for MemoRefHead{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MemoRefHead::Null       => {
                fmt.debug_struct("MemoRefHead::Null").finish()
            },
            MemoRefHead::Anonymous{ ref head, .. } => {
                fmt.debug_struct("MemoRefHead::Anonymous")
                    .field("memo_refs",  head )
                    //.field("memo_ids", &self.memo_ids() )
                    .finish()
            }
            MemoRefHead::Subject{ ref subject_id, ref head } => {
                fmt.debug_struct("MemoRefHead::Subject")
                    .field("subject_id", &subject_id )
                    .field("memo_refs",  head )
                    //.field("memo_ids", &self.memo_ids() )
                    .finish()
            }
        }
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
    pub fn from_memoref (memoref: &MemoRef, slab: &Slab ) -> Self {
        let mut q = VecDeque::new();
        q.push_front(memoref.clone());

        CausalMemoIter {
            queue: q,
            slab:  slab.clone()
        }
    }
}
impl Iterator for CausalMemoIter {
    type Item = Result<Memo,RetrieveError>;

    fn next (&mut self) -> Option<Result<Memo,RetrieveError>> {
        // iterate over head memos
        // Unnecessarly complex because we're not always dealing with MemoRefs
        // Arguably heads should be stored as Vec<MemoRef> instead of Vec<Memo>

        // TODO: Stop traversal when we come across a Keyframe memo
        if let Some(memoref) = self.queue.pop_front() {
            // this is wrong - Will result in G, E, F, C, D, B, A

            match memoref.get_memo( &self.slab ) {
                Ok(memo) => {
                    self.queue.append(&mut memo.get_parent_head().to_vecdeque());
                    return Some(Ok(memo));
                },
                Err(e) => return Some(Err(e))
            }
        }

        return None;
    }
}
