mod projection;

use slab::prelude::*;
use subject::*;
use error::*;

use futures::{self,prelude::*};
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
    pub fn memo_ids (&self) -> Vec<MemoId> {
        match *self {
            MemoRefHead::Null => Vec::new(),
            MemoRefHead::Subject{ ref head, .. } | MemoRefHead::Anonymous{ ref head, .. } => head.iter().map(|m| m.memo_id).collect()
        }
    }
    pub fn subject_id (&self) -> SubjectId {
        match *self {
            MemoRefHead::Null | MemoRefHead::Anonymous{..} => SubjectId::anonymous(),
            MemoRefHead::Subject{ subject_id, .. }         => subject_id
        }
    }
    pub fn is_some (&self) -> bool {
        match *self {
            MemoRefHead::Null => false,
            _                 => true
        }
    }
    pub fn to_vec (self) -> Vec<MemoRef> {
        match *self {
            MemoRefHead::Null => vec![],
            MemoRefHead::Anonymous { head, .. } | MemoRefHead::Subject{  head, .. }   => head
        }
    }
    pub fn to_vecdeque (&self) -> VecDeque<MemoRef> {
        match *self {
            MemoRefHead::Null       => VecDeque::new(),
            MemoRefHead::Anonymous { ref head, .. } | MemoRefHead::Subject{  ref head, .. }   => VecDeque::from(head.clone())
        }
    }
    pub fn len (&self) -> usize {
        match *self {
            MemoRefHead::Null       =>  0,
            MemoRefHead::Anonymous { ref head, .. } | MemoRefHead::Subject{  ref head, .. }   => head.len()
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn iter (&self) -> slice::Iter<MemoRef> {

        // This feels pretty stupid. Probably means something is wrong with the factorization of MRH
        static EMPTY : &'static [MemoRef] = &[];

        match *self {
            MemoRefHead::Null                    => EMPTY.iter(), // HACK
            MemoRefHead::Anonymous{ ref head } | MemoRefHead::Subject{ ref head, .. } => head.iter()
        }
    }
    pub fn clone_for_slab (&self, from_slab: &mut LocalSlabHandle, to_slab: &LocalSlabHandle, include_memos: bool ) -> Self {
        assert!(from_slab.slab_id != to_slab.slab_id(), "slab id should differ");
        match *self {
            MemoRefHead::Null                    => MemoRefHead::Null,
            MemoRefHead::Anonymous { ref head }  => MemoRefHead::Anonymous{
                head: head.iter().map(|mr| mr.clone_for_slab(from_slab, to_slab, include_memos )).collect()
            },
            MemoRefHead::Subject{ subject_id, ref head } => MemoRefHead::Subject {
                subject_id: subject_id,
                head:       head.iter().map(|mr| mr.clone_for_slab(from_slab, to_slab, include_memos )).collect()
            }
        }
    }

}

use std::rc::Rc;
use std::cell::RefCell;

#[derive(Clone)]
pub struct MemoRefHeadOuter(pub Rc<RefCell<MemoRefHead>>);

impl MemoRefHeadOuter {
    pub fn apply_memoref(&self, new: MemoRef, slab: &LocalSlabHandle ) -> Box<Future<Item=bool, Error=Error>> {
        //println!("# MemoRefHead({:?}).apply_memoref({})", self.memo_ids(), &new.id);

        // Conditionally add the new memoref only if it descends any memorefs in the head
        // If so, any memorefs that it descends must be removed
        let head = match *self.0.borrow() {
            MemoRefHead::Null => {
                if !new.subject_id.is_anonymous() {
                    *self = MemoRefHead::Subject{
                        head: vec![new.clone()],
                        subject_id: new.subject_id
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

        unimplemented!()
        // TODO: Convert this to Futures
//        'existing: for i in (0..head.len()).rev() {
//            let mut remove = false;
//            {
//                let ref mut existing: &mut MemoRef = head[i];
//                if existing == new {
//                    return Ok(false); // we already had this
//
//
//                } else if existing.descends(&new,&slab).wait()? {
//                    new_is_descended = true;
//
//                    // IMPORTANT: for the purposes of the boolean return,
//                    // the new memo does not get "applied" in this case
//
//                    // If any memo in the head already descends the newcomer,
//                    // then it doesn't get applied at all punt the whole thing
//                    break 'existing;
//
//                } else if new.descends(&existing, &slab).wait()? {
//                    new_descends = true;
//                    applied = true; // descends
//
//                    if replaced {
//                        remove = true;
//                    }else{
//                        // Lets try real hard not to remove stuff in the middle of the vec
//                        // But we only get to do this trick once, because we don't want to add duplicates
//                        mem::replace( existing, new.clone() );
//                        replaced = true;
//                    }
//
//                }
//            }
//
//            if remove {
//                // because we're descending, we know the offset of the next items won't change
//                head.remove(i);
//            }
//        }
//
//        if !new_descends && !new_is_descended  {
//            // if the new memoref neither descends nor is descended
//            // then it must be concurrent
//
//            head.push(new.clone());
//            applied = true; // The memoref was "applied" to the MemoRefHead
//        }
//
//        // This memoref was applied if it was concurrent, or descends one or more previous memos
//
//        if applied {
//            //println!("# \t\\ Was applied - {:?}", self.memo_ids());
//        }else{
//            //println!("# \t\\ NOT applied - {:?}", self.memo_ids());
//        }
//
//       Ok(applied)
    }
    pub fn apply_memorefs (&self, new_memorefs: &Vec<MemoRef>, slab: &LocalSlabHandle) -> Result<(),Error> {
        for new in new_memorefs.iter(){
            self.apply_memoref(new, slab)?;
        }
        Ok(())
    }
    pub fn apply (&self, other: MemoRefHead, slab: LocalSlabHandle) -> Box<Future<Item=bool,Error=Error>> {
        let mut applied = false;

        let self_dup = self.clone();

        futures::stream::iter_ok::<MemoRef, ()>(other.to_vec().drain(..)).fold(false, move |acc, memoref|{
            self_dup.apply_memoref( memoref, slab ).and_then(|applied| {
                if applied {
                    *acc = true;
                }
            })
        })
    }

    /// Test to see if this MemoRefHead fully descends another
    /// If there is any hint of causal concurrency, then this will return false
    pub fn descends_or_contains (&self, other: &MemoRefHead, slab: &LocalSlabHandle) -> Result<bool,Error> {

        // there's probably a more efficient way to do this than iterating over the cartesian product
        // we can get away with it for now though I think
        // TODO: revisit when beacons are implemented
        match *self {
            MemoRefHead::Null             => Ok(false),
            MemoRefHead::Subject{ ref head, .. } | MemoRefHead::Anonymous{ ref head, .. } => {
                match *other {
                    MemoRefHead::Null             => Ok(false),
                    MemoRefHead::Subject{ head: ref other_head, .. } | MemoRefHead::Anonymous{ head: ref other_head, .. } => {
                        if head.is_empty() || other_head.is_empty() {
                            return Ok(false) // searching for positive descendency, not merely non-ascendency
                        }
                        for memoref in head.iter(){
                            'other: for other_memoref in other_head.iter(){
                                if memoref == other_memoref {
                                    //
                                } else if !memoref.descends(other_memoref, slab).wait()? {
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

    pub fn causal_memo_iter(&self, slab: &LocalSlabHandle ) -> CausalMemoIter {
        CausalMemoIter::from_head( self, slab )
    }
    pub fn is_fully_materialized(&self, slab: &LocalSlabHandle ) -> bool {
        // TODO: consider doing as-you-go distance counting to the nearest materialized memo for each descendent
        //       as part of the list management. That way we won't have to incur the below computational effort.

        let head = match *self {
            MemoRefHead::Null       => {
                return true;
            },
            MemoRefHead::Anonymous { ref head, .. } | MemoRefHead::Subject{  ref head, .. } => head
        };

        for memoref in head.iter(){
            // TODO: update head iter to be a stream
            let memo = slab.get_memo(memoref.clone(), true ).wait().unwrap();

            if let MemoBody::FullyMaterialized { .. } = memo.body {
                //
            }else{
                return false
            }
        }

        true
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
    slab:  LocalSlabHandle
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
    pub fn from_head ( head: &MemoRefHead, slab: &LocalSlabHandle) -> Self {
        //println!("# -- SubjectMemoIter.from_head({:?})", head.memo_ids() );

        CausalMemoIter {
            queue: head.to_vecdeque(),
            slab:  slab.clone()
        }
    }
    pub fn from_memoref (memoref: &MemoRef, slab: &LocalSlabHandle ) -> Self {
        let mut q = VecDeque::new();
        q.push_front(memoref.clone());

        CausalMemoIter {
            queue: q,
            slab:  slab.clone()
        }
    }
}
impl Iterator for CausalMemoIter {
    type Item = Result<Memo,Error>;

    fn next (&mut self) -> Option<Result<Memo,Error>> {
        // iterate over head memos
        // Unnecessarly complex because we're not always dealing with MemoRefs
        // Arguably heads should be stored as Vec<MemoRef> instead of Vec<Memo>

        // TODO: Stop traversal when we come across a Keyframe memo
        if let Some(memoref) = self.queue.pop_front() {
            // this is wrong - Will result in G, E, F, C, D, B, A

            match self.slab.get_memo( memoref.clone(), true ).wait() {
                Ok(memo) => {
                    self.queue.append(&mut memo.get_parent_head().to_vecdeque());
                    return Some(Ok(memo));
                },
                Err(e) => return Some(Err(e))
            }
        }

        None
    }
}
