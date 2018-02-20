pub mod receiver;

pub use serde::ser::Serialize;
pub use serde::de::Deserialize;

use network::TransportAddress;
use subject::SubjectId;
use slab::prelude::*;

use self::receiver::BufferReceiver;

#[derive(Serialize,Deserialize)]
pub struct NetworkBuffer{
    subjects: Vec<SubjectId>,
    slabrefs: Vec<SlabRefBuffer>,
    memorefs: Vec<MemoRefBuffer>,
    memos: Vec<MemoBuffer>
}

type SubjectOffset = u16;
type SlabRefOffset = u16;
type MemoRefOffset = u16;

/// MemoId, offset of the Memo's subject id, list of slab presence offsets wherein this memo is purportedly present, list of slab presence offsets wherein this memo is peered but not present
struct MemoRefBuffer( MemoId, SubjectOffset, Vec<SlabPresenceOffset>, Vec<SlabPresenceOffset>);
struct MemoBuffer( MemoRefOffset, Vec<MemoRefOffset>, MemoBodyBuffer );
enum MemoBodyBuffer {
    SlabPresence{ p: SlabRefOffset, r: Vec<MemoRefOffset> }, // TODO: split out root_index_seed conveyance to another memobody type
    Edge(EdgeSetBuffer),
    Edit(HashMap<String, String>),
    FullyMaterialized     { v: HashMap<String, String>, e: EdgeSetBuffer },
    PartiallyMaterialized { v: HashMap<String, String>, e: EdgeSetBuffer },
    Peering(MemoRefOffset,SubjectOffset,Vec<MemoPeerStateBuffer>),
    MemoRequest(Vec<MemoRefOffset>,Vec<SlabRefOffset>)
}

struct SlabRefBuffer{
    slab_id: SlabId,
    addresses: Vec<TransportAddress>,
    anticipated_lifetime: SlabAnticipatedLifetime
}

struct MemoPeerStateBuffer {
    slab: SlabRefOffset,
    status: MemoPeerStatus
}

pub struct EdgeSetBuffer (pub HashMap<RelationSlotId, Vec<MemoRefOffset>>);


impl NetworkBuffer{
    pub fn from_vec(vec: Vec<u8>) -> Self {
        // TODO: Serde guts
        unimplemented!()
    }
    fn extract_to( receiver: impl BufferReceiver ){

    }
}