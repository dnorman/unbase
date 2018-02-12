/* Memo
 * A memo is an immutable message.
*/
pub mod serde;

use std::collections::HashMap;
use std::{fmt};
use futures::prelude::*;
use futures::future;

use subject::{SubjectId,SubjectType};
use slab::prelude::*;
use memorefhead::MemoRefHead;
use error::*;

//pub type MemoId = [u8; 32];
pub type MemoId = u64;

// All portions of this struct should be immutable – but not necessarily identical to other copies of the memo on other slabs
// This is because the MemoRefs and SlabRefs might differ in precise content, even though they are semantically (and hashvalue) identical
#[derive(Clone)]
pub struct Memo {
    pub id: u64,
    pub subject_id: Option<SubjectId>,
    pub owning_slabref: SlabRef,
    pub parents: MemoRefHead,
    pub body: MemoBody
}



#[derive(Clone, Debug)]
pub enum MemoBody{
    SlabPresence{ p: SlabPresence, r: MemoRefHead }, // TODO: split out root_index_seed conveyance to another memobody type
    Relation(RelationSet),
    Edge(EdgeSet),
    Edit(HashMap<String, String>),
    FullyMaterialized     { v: HashMap<String, String>, r: RelationSet, e: EdgeSet, t: SubjectType },
    PartiallyMaterialized { v: HashMap<String, String>, r: RelationSet, e: EdgeSet, t: SubjectType },
    Peering(MemoId,Option<SubjectId>,Vec<MemoPeerState>),
    MemoRequest(Vec<MemoRef>,SlabPresence)
}


/*
use std::hash::{Hash, Hasher};

impl Hash for MemoId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.originSlab.hash(state);
        self.id.hash(state);
    }
}
*/

impl fmt::Debug for Memo{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Memo")
           .field("id", &self.id)
           .field("subject_id", &self.subject_id)
           .field("parents", &self.parents)
           .field("body", &self.body)
           .finish()
    }
}

impl Memo {
    pub fn get_parent_head (&self) -> MemoRefHead {
        self.parents.clone()
    }
    pub fn get_values (&self) -> Option<(HashMap<String, String>,bool)> {

        match self.body {
            MemoBody::Edit(ref v)
                => Some((v.clone(),false)),
            MemoBody::FullyMaterialized { ref v, .. }
                => Some((v.clone(),true)),
            _   => None
        }
    }
    pub fn get_relations (&self) -> Option<(RelationSet,bool)> {

        match self.body {
            MemoBody::Relation(ref r)
                => Some((r.clone(),false)),
            MemoBody::FullyMaterialized { ref r, .. }
                => Some((r.clone(),true)),
            _   => None
        }
    }
    pub fn get_edges (&self) -> Option<(EdgeSet,bool)> {

        match self.body {
            MemoBody::Edge(ref e)
                => Some((e.clone(),false)),
            MemoBody::FullyMaterialized { ref e, .. }
                => Some((e.clone(),true)),
            _   => None
        }
    }
    pub fn does_peering (&self) -> bool {
        match self.body {
            MemoBody::MemoRequest(_,_) => {
                false
            }
            MemoBody::Peering(_,_,_) => {
                false
            }
            MemoBody::SlabPresence{p:_, r:_} => {
                false
            }
            _ => {
                true
            }
        }
    }
    pub fn descends (&self, memoref: &MemoRef, slab: &LocalSlabHandle) -> Box<Future<Item=bool, Error=Error>> {
        //TODO: parallelize this
        //TODO: Use sparse-vector/beacon to avoid having to trace out the whole lineage
        //      Should be able to stop traversal once happens-before=true. Cannot descend a thing that happens after

        // breadth-first
        for parent in self.parents.iter() {
            if parent == memoref {
                return Box::new(future::result(Ok(true)))
            };
        }

        // Ok now depth - TODO: join parent futures
        for parent in self.parents.iter() {
            match parent.descends(&memoref,slab).wait() {
                Ok(true)  => return Box::new(future::result(Ok(true))),
                Ok(false) => continue,
                Err(e)    => Box::new(future::result(Err(e)))
            };
        }
        return Box::new(future::result(Ok(false)));
    }
    pub fn clone_for_slab (&self, from_slabref: &SlabRef, to_slab: &LocalSlabHandle, peerlist: &Vec<MemoPeerState>) -> Memo {
        assert!(from_slabref.owning_slab_id == to_slab.id, "Memo clone_for_slab owning slab should be identical");

        let memo = Memo{
            id:             self.id,
            owning_slabref: to_slab.slabref,
            subject_id:     self.subject_id,
            parents:        self.parents.clone_for_slab(from_slabref, to_slab, false),
            body:           self.body.clone_for_slab(from_slabref, to_slab)
        };

        to_slab.receive_memo_with_peerlist( memo.clone(), peerlist.clone(), from_slabref.clone() );
        
        memo
    }
}

impl MemoBody {
    fn clone_for_slab(&self, from_slabref: &SlabRef, to_slab: &LocalSlabHandle ) -> MemoBody {
        assert!(from_slabref.owning_slab_id == to_slab.id, "MemoBody clone_for_slab owning slab should be identical");

        match self {
            &MemoBody::SlabPresence{ ref p, ref r } => {
                MemoBody::SlabPresence{
                    p: p.clone(),
                    r: r.clone_for_slab(from_slabref, to_slab, true),

                    // match r {
                    //     &MemoRefHead::Subject{..} | &MemoRefHead::Anonymous{..} => {
                    //         r.clone_for_slab(from_slabref, to_slab, true)
                    //     }
                    //     &MemoRefHead::Null => MemoRefHead::Null
                    // }
                }
            }
            &MemoBody::Relation(ref relationset) => {
                MemoBody::Relation(relationset.clone_for_slab(from_slabref, to_slab))
            }
            &MemoBody::Edge(ref edgeset) => {
                MemoBody::Edge(edgeset.clone_for_slab(from_slabref, to_slab))
            }
            &MemoBody::Edit(ref hm) => {
                MemoBody::Edit(hm.clone())
            }
            &MemoBody::FullyMaterialized{ ref v, ref r, ref t, ref e } => {
                MemoBody::FullyMaterialized{ v: v.clone(), r: r.clone_for_slab(from_slabref, to_slab), e: e.clone_for_slab(from_slabref, to_slab), t: t.clone() }
            }
            &MemoBody::PartiallyMaterialized{ ref v, ref r,ref e, ref t } => {
                MemoBody::PartiallyMaterialized{ v: v.clone(), r: r.clone_for_slab(from_slabref, to_slab), e: e.clone_for_slab(from_slabref, to_slab), t: t.clone() }
            }
            &MemoBody::Peering(memo_id, subject_id, ref peerlist) => {
                MemoBody::Peering(memo_id,subject_id,peerlist.clone_for_slab(to_slab))
            }
            &MemoBody::MemoRequest(ref memo_ids, ref slabref) =>{
                MemoBody::MemoRequest(memo_ids.clone(), to_slab.slabref_for_slab_id(slabref.slab_id()))
            }
        }

    }
}
