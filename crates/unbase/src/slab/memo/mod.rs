// Memo
// A memo is an immutable message.
pub mod serde;

use core::ops::Deref;
use futures::future::{
    BoxFuture,
    FutureExt,
};
use std::{
    collections::HashMap,
    fmt,
    sync::Arc,
};

use crate::{
    buffer::{
        BufferHelper,
        EditBufElement,
        MemoBodyBufElement,
        MemoBuf,
        RelationSetBufElement,
    },
    error::RetrieveError,
    head::Head,
    network::{
        SlabPresence,
        SlabRef,
    },
    slab::{
        EdgeSet,
        EntityId,
        EntityType,
        MemoPeerList,
        MemoRef,
        RelationSet,
        SlabHandle,
        SlabId,
    },
};
use ::serde::{
    Deserialize,
    Serialize,
};
use itertools::Itertools;

// pub type MemoId = [u8; 32];
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
    pub entity_id:      Option<EntityId>,
    pub owning_slabref: SlabRef,
    pub parents:        Head,
    pub body:           MemoBody,
}

#[derive(Clone, Debug)]
pub enum MemoBody {
    SlabPresence {
        p: SlabPresence,
        r: Head,
    }, // TODO: split out root_index_seed conveyance to another memobody type
    Relation(RelationSet),
    Edge(EdgeSet),
    Edit(HashMap<String, String>),
    FullyMaterialized {
        v: HashMap<String, String>,
        r: RelationSet,
        e: EdgeSet,
        t: EntityType,
    },
    PartiallyMaterialized {
        v: HashMap<String, String>,
        r: RelationSet,
        e: EdgeSet,
        t: EntityType,
    },
    Peering(MemoId, Option<EntityId>, MemoPeerList),
    MemoRequest(Vec<MemoId>, SlabRef),
}

// use std::hash::{Hash, Hasher};
//
// impl Hash for MemoId {
// fn hash<H: Hasher>(&self, state: &mut H) {
// self.originSlab.hash(state);
// self.id.hash(state);
// }
// }

impl fmt::Debug for Memo {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Memo")
           .field("id", &self.id)
           .field("entity_id", &self.entity_id)
           .field("parents", &self.parents)
           .field("body", &self.body)
           .finish()
    }
}

impl Memo {
    pub fn new(inner: MemoInner) -> Self {
        Memo(Arc::new(inner))
    }

    pub fn serialize(&self, _slab: &SlabHandle) {
        // -> MemoBuf {
        unimplemented!()
    }

    pub fn get_parent_head(&self) -> Head {
        self.parents.clone()
    }

    pub fn get_values(&self) -> Option<(HashMap<String, String>, bool)> {
        match self.body {
            MemoBody::Edit(ref v) => Some((v.clone(), false)),
            MemoBody::FullyMaterialized { ref v, .. } => Some((v.clone(), true)),
            _ => None,
        }
    }

    pub fn get_relations(&self) -> Option<(RelationSet, bool)> {
        match self.body {
            MemoBody::Relation(ref r) => Some((r.clone(), false)),
            MemoBody::FullyMaterialized { ref r, .. } => Some((r.clone(), true)),
            _ => None,
        }
    }

    pub fn get_edges(&self) -> Option<(EdgeSet, bool)> {
        match self.body {
            MemoBody::Edge(ref e) => Some((e.clone(), false)),
            MemoBody::FullyMaterialized { ref e, .. } => Some((e.clone(), true)),
            _ => None,
        }
    }

    pub fn does_peering(&self) -> bool {
        match self.body {
            MemoBody::MemoRequest(_, _) => false,
            MemoBody::Peering(_, _, _) => false,
            MemoBody::SlabPresence { p: _, r: _ } => false,
            _ => true,
        }
    }

    #[tracing::instrument]
    pub fn descends<'a>(&'a self, memoref: &'a MemoRef, slab: &'a SlabHandle) -> BoxFuture<'a, Result<bool, RetrieveError>> {
        // Not really sure if this is right

        // TODO: parallelize this
        // TODO: Use sparse-vector/beacon to avoid having to trace out the whole lineage
        //      Should be able to stop traversal once happens-before=true. Cannot descend a thing that happens after

        async move {
            // breadth-first
            for parent in self.parents.iter() {
                if parent == memoref {
                    return Ok(true);
                };
            }
            // Ok now depth
            for parent in self.parents.iter() {
                if parent.descends(&memoref, slab).await? {
                    return Ok(true);
                }
            }
            return Ok(false);
        }.boxed()
    }

    pub fn to_buf<E, M, S, H: BufferHelper + Sized>(&self, helper: &H) -> MemoBuf<E, M, S>
        where E: Serialize + Deserialize,
              M: Serialize + Deserialize,
              S: Serialize + Deserialize
    {
        MemoBuf::<E, M> { entity_id: helper.from_entity_id(self.entity_id),
                          parents:   self.parents.to_buf(helper),
                          body:      self.body.to_buf(helper), }
    }
}

impl MemoBody {
    pub fn summary(&self) -> String {
        use MemoBody::*;

        match self {
            SlabPresence { ref p, ref r } => {
                if r.is_some() {
                    format!("SlabPresence({} at {})*", p.slabref, p.address.to_string())
                } else {
                    format!("SlabPresence({} at {})", p.slabref, p.address.to_string())
                }
            },
            Relation(ref rel_set) => format!("RelationSet({})", rel_set.to_string()),
            Edge(ref _edge_set) => format!("EdgeSet"),
            Edit(ref _e) => format!("Edit"),
            FullyMaterialized { .. } => format!("FullyMaterialized"),
            PartiallyMaterialized { .. } => format!("PartiallyMaterialized"),
            Peering(ref _memo_id, ref _entity_id, ref _peerlist) => format!("Peering"),
            MemoRequest(ref memo_ids, ref slabref) => {
                format!("MemoRequest({} to {})", memo_ids.iter().join(","), slabref.slab_id)
            },
        }
    }

    pub fn to_buf<E, M, S, H: BufferHelper + Sized>(&self, helper: &H) -> MemoBodyBufElement<E, M, S>
        where E: Serialize + Deserialize,
              M: Serialize + Deserialize,
              S: Serialize + Deserialize
    {
        match self {
            MemoBody::SlabPresence { ref p, ref r } => {
                //                MemoBody::SlabPresence { p: p.clone(),
                //                    r: self.localize_head(r, from_slabref, true), }
                unimplemented!()
            },
            MemoBody::Relation(ref relationset) => MemoBodyBufElement::Relation(relationset.to_buf(helper)),
            MemoBody::Edge(ref edgeset) => MemoBodyBufElement::Edge(edgeset.to_buf()),
            MemoBody::Edit(ref hm) => MemoBodyBufElement::Edit(EditBufElement { edit: (*hm).clone() }),
            MemoBody::FullyMaterialized { ref v,
                                          ref r,
                                          ref t,
                                          ref e, } => {
                //                MemoBodyBufElement::FullyMaterialized { v: (*v).clone(),
                //                    r: RelationSetBufElement{ slots: },
                //                    e: self.localize_edgeset(e, from_slabref),
                //                    t: t.clone(), }
                unimplemented!()
            },
            MemoBody::PartiallyMaterialized { ref v,
                                              ref r,
                                              ref e,
                                              ref t, } => {
                //                MemoBody::PartiallyMaterialized { v: v.clone(),
                //                    r: r.clone(),
                //                    e: self.localize_edgeset(e, from_slabref),
                //                    t: t.clone(), }
                unimplemented!()
            },

            MemoBody::Peering(memo_id, entity_id, ref peerlist) => {
                MemoBody::Peering(memo_id, entity_id, self.localize_peerlist(peerlist))
            },
            MemoBody::MemoRequest(ref memo_ids, ref slabref) => {
                //                MemoBodyBufElement::MemoRequest(memo_ids.clone(), self.localize_slabref(slabref))
            },
        }
    }
}
