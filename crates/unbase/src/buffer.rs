use std::collections::HashMap;

use crate::slab::{EntityId, MemoId, SlabPresence, MemoBody, SlabId, MemoPeeringStatus};
use crate::head::Head;

struct SlabBuf{
    slab_id: SlabId,
    presence: Vec<SlabPresence>,
    // may have other non-presence stuff here, like stats, or context-specific interpretations of the SlabPresence perhaps
}

/// Intentionally not including MemoId here, because it's based on this content
/// If we're actually storing this we will need to calculate the hash though.
///
/// If we're actually using this for local storage, then I suppose it doesn't much matter if we're
struct MemoBuf{
    entity_id: EntityId, // use Offset for net version
    parents: Vec<MemoId>, // use Vec<Offset> for net version
    body: MemoBodyBuf
}

/// Directly Stored in slab::State
/// We can key this on MemoId in the lookup, and thus not have to store remote memos at all
struct MemoPeersBuf {
    memo_id: MemoId, // use Offset in net version
    peers: Vec<MemoPeerBuf>, // use Vec<Offset> in net version
}


struct MemoPeerElement {
    slab_id: SlabId, // use Offset in net version
    status: MemoPeeringStatus,
}



#[derive(Serialize,Deserialize,Clone)]
pub struct EdgeSetElement (pub HashMap<RelationSlotId, Vec<MemoRefOffset>>);

// TODO
//pub struct EdgeSetElement{}
//pub struct ValueSetElement{}

#[derive(Serialize,Deserialize,Clone)]
pub enum MemoBodyElement {
    // Arguably we shouldn't ever be storing a SlabPresence, Peering, or MemoRequest Memo to disk, so consider removing these entirely
    //    SlabPresence{ p: SlabPresenceBuffer, r: Vec<MemoRefOffset> }, // TODO: split out root_index_seed conveyance to another memobody type
    //    Peering(MemoRefOffset,Vec<MemoPeerStateBuffer>),
    //    MemoRequest(Vec<MemoRefOffset>,Vec<SlabRefOffset>)

    Edge(EdgeSetElement),
    Edit(HashMap<String, String>), // TODO convert this to EditBodyElement
    FullyMaterialized     { v: HashMap<String, String>, e: EdgeSetElement },
    PartiallyMaterialized { v: HashMap<String, String>, e: EdgeSetElement }, // TODO convert v to ValueSetElement
}