use crate::slab::EntityId;
use std::sync::Arc;

pub struct MemoRef(pub Arc<MemoRefInner>);

pub struct MemoRefInner {
//    pub id:             MemoId,
//    pub owning_slab_id: SlabId, // TODO - rename and conditionalize with a macro
//    pub entity_id:      Option<EntityId>,
//    pub peerlist:       RwLock<MemoPeerList>,
//    pub ptr:            RwLock<MemoRefPtr>,
}

pub enum MemoRefPtr {
    Resident(Memo),
    Remote,
}

pub enum Head {
    Null,
    Entity {
        entity_id:      EntityId,
        head:           Vec<MemoRef>,
    },
    Anonymous {
        head:           Vec<MemoRef>,
    },
}

// TODO - zerocopy
pub struct Memo {
    pub parents:        Head,
    pub body:           (),
}