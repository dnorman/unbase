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
    error::RetrieveError,
    head::Head,
    network::{
        SlabPresence,
        SlabRef,
    },
    slab::{
        state::SlabStateBufHelper,
        EdgeSet,
        EntityId,
        EntityType,
        MemoPeerList,
        MemoRef,
        RelationSet,
        SlabHandle,
    },
};

use itertools::Itertools;

use tracing::warn;

#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct MemoId([u8; 32]);

impl std::convert::AsRef<[u8]> for MemoId {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}
impl MemoId {
    pub fn dummy() -> Self {
        Self([0u8; 32])
    }

    pub fn base64(&self) -> String {
        use base64::encode;

        // Don't use this too much. It allocs!
        encode(&self.0)
    }

    pub fn short(&self) -> String {
        self.base64()[0..6].to_string()
    }
}

impl std::fmt::Display for MemoId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.base64()[..6])
    }
}

impl std::fmt::Debug for MemoId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("MemoId").field("", &self.base64()).finish()
    }
}

// All portions of this struct should be immutable

#[derive(Clone)]
pub struct Memo(pub Arc<MemoInner>);

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

    pub fn id(&self) -> MemoId {
        warn!("Stop using MemoId->id()! Only the storage engine should be calculating MemoId");

        let buf = self.to_buf(&SlabStateBufHelper {});
        let bytes: Vec<u8> = bincode::serialize(&buf).unwrap();

        let mut hasher = sha2::Sha256::new();
        hasher.input(&bytes);

        let hash = hasher.result();

        MemoId(hash.into())
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
}

impl MemoBody {
    pub fn summary(&self) -> String {
        use MemoBody::*;

        match self {
            SlabPresence { ref p, ref r } => {
                if r.is_some() {
                    format!("SlabPresence({} at {})*", p.slab_id, p.address.to_string())
                } else {
                    format!("SlabPresence({} at {})", p.slab_id, p.address.to_string())
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
}
