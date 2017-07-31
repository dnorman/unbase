/* Memo
 * A memo is an immutable message.
*/
pub mod serde;

use std::collections::HashMap;
use std::{fmt};
use std::sync::Arc;

use subject::{SubjectId,SubjectType};
use slab::MemoRef;
use network::{SlabRef,SlabPresence};
use super::*;

//pub type MemoId = [u8; 32];
pub type MemoId = u64;

// All portions of this struct should be immutable

#[derive(Clone)]
pub struct Memo(Arc<MemoInner>);

impl Deref for Memo {
    type Target = MemoInner;
    fn deref(&self) -> &MemoInner {
        &*self.0
    }
}

pub struct MemoInner {
    pub id: u64,
    pub subject_id: Option<SubjectId>,
    pub owning_slab_id: SlabId,
    pub parents: MemoRefHead,
    pub body: MemoBody
}

#[derive(Clone, Debug)]
pub enum MemoBody{
    SlabPresence{ p: SlabPresence, r: Option<MemoRefHead> }, // TODO: split out root_index_seed conveyance to another memobody type
    Relation(RelationSet),
    Edge(EdgeSet),
    Edit(HashMap<String, String>),
    FullyMaterialized     { v: HashMap<String, String>, r: RelationSet, e: EdgeSet, t: SubjectType },
    PartiallyMaterialized { v: HashMap<String, String>, r: RelationSet, e: EdgeSet, t: SubjectType },
    Peering(MemoId,Option<SubjectId>,MemoPeerList),
    MemoRequest(Vec<MemoId>,SlabRef)
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
    pub fn new (inner: MemoInner) -> Self {
        Memo(Arc::new(inner))
    }
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
    pub fn descends (&self, memoref: &MemoRef, slab: &Slab) -> bool {
        //TODO: parallelize this
        //TODO: Use sparse-vector/beacon to avoid having to trace out the whole lineage
        //      Should be able to stop traversal once happens-before=true. Cannot descend a thing that happens after

        // breadth-first
        for parent in self.parents.iter() {
            if parent == memoref {
                return true
            };
        }

        // Ok now depth
        for parent in self.parents.iter() {
            if parent.descends(&memoref,slab) {
                return true
            }
        }
        return false;
    }
    pub fn clone_for_slab (&self, from_slabref: &SlabRef, to_slab: &Slab, peerlist: &MemoPeerList) -> Memo {
        assert!(from_slabref.owning_slab_id == to_slab.id, "Memo clone_for_slab owning slab should be identical");

        //println!("Slab({}).Memo.clone_for_slab(memo: {}, from: {}, to: {}, peers: {:?})", self.owning_slab_id, self.id, from_slabref.slab_id, to_slab.id, peerlist );
        to_slab.reconstitute_memo(
            self.id,
            self.subject_id,
            self.parents.clone_for_slab(from_slabref, to_slab, false),
            self.body.clone_for_slab(from_slabref, to_slab),
            from_slabref,
            peerlist
        ).0
    }
}

impl MemoBody {
    fn clone_for_slab(&self, from_slabref: &SlabRef, to_slab: &Slab ) -> MemoBody {
        assert!(from_slabref.owning_slab_id == to_slab.id, "MemoBody clone_for_slab owning slab should be identical");

        match self {
            &MemoBody::SlabPresence{ ref p, ref r } => {
                MemoBody::SlabPresence{
                    p: p.clone(),
                    r: match r {
                        &Some(ref root_mrh) => {
                            Some(root_mrh.clone_for_slab(from_slabref, to_slab, true))
                        }
                        &None => None
                    }
                }
            },
            &MemoBody::Relation(ref rssh) => {
                MemoBody::Relation(rssh.clone_for_slab(from_slabref, to_slab))
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
                MemoBody::MemoRequest(memo_ids.clone(), slabref.clone_for_slab(to_slab))
            }
        }

    }
}
