use std::{
    collections::HashMap,
    fmt,
    ops::Deref,
};

use crate::{
    head::Head,
    network::{
        SlabRef,
        TransportAddress,
    },
    slab::SlabId,
};
use itertools::Itertools;

pub const MAX_SLOTS: usize = 256;
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct IndexInfo {
    pub(crate) tier: u8,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub enum EntityType {
    IndexNode(IndexInfo),
    Record,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Serialize, Deserialize)]
pub struct EntityId {
    pub id:    u64,
    pub etype: EntityType,
}
impl<'a> core::cmp::PartialEq<&'a str> for EntityId {
    fn eq(&self, other: &&'a str) -> bool {
        self.concise_string() == *other
    }
}

impl EntityId {
    pub fn test(test_id: u64) -> Self {
        EntityId { id:    test_id,
                   etype: EntityType::Record, }
    }

    /// Create a EntityId with a EntityType of IndexNode and a manually provided id
    /// Used by the test suite
    pub fn index_test(test_id: u64) -> Self {
        EntityId { id:    test_id.into(),
                   etype: EntityType::IndexNode(IndexInfo { tier: 255 }), }
    }

    pub fn id_short(&self) -> String {
        // TODO - this should be the first 6 characters of the MemoId hash
        format!("{}", self.id)
    }

    /// Human readable version of the EntityID which denotes whether the entity is an (I)ndex or a (R)ecord type
    pub fn concise_string(&self) -> String {
        match self.etype {
            EntityType::IndexNode(info) => format!("I{}", info.tier),
            EntityType::Record => format!("R{}", self.id_short()),
        }
    }
}

impl fmt::Display for EntityId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.concise_string())
    }
}

/// SlabPresence represents the expected reachability of a given Slab
/// Including Transport address and anticipated lifetime
#[derive(Clone, Serialize, Deserialize)]
pub struct SlabPresence {
    // TODO NEXT - get rid of SlabPresence entirely. it shouldn't exist except as a Buffer
    pub slab_id:  SlabId,
    pub address:  TransportAddress,
    pub liveness: TransportLiveness,
    //    pub latest_clock: Head, // latest clock reading relating to this slab presence
}
impl PartialEq for SlabPresence {
    fn eq(&self, other: &SlabPresence) -> bool {
        // When comparing equality, we can skip the anticipated lifetime
        self.slab_id == other.slab_id && self.address == other.address
    }
}
impl fmt::Debug for SlabPresence {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("SlabPresence")
           .field("slab_id", &self.slab_id)
           .field("address", &self.address.to_string())
           .field("liveness", &self.liveness)
           .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TransportLiveness {
    Available,
    Unavailable,
    Unknown,
    // Could be other stuff
}

impl TransportLiveness {
    pub fn is_available(&self) -> bool {
        match self {
            TransportLiveness::Available => true,
            _ => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MemoPeerList(pub Vec<MemoPeer>);

impl MemoPeerList {
    pub fn new(list: Vec<MemoPeer>) -> Self {
        MemoPeerList(list)
    }

    pub fn clone(&self) -> Self {
        MemoPeerList(self.0.clone())
    }

    pub fn slab_ids(&self) -> Vec<SlabId> {
        self.0.iter().map(|p| p.slabref.id().clone()).collect()
    }

    pub fn apply_peer(&mut self, peer: MemoPeer) -> bool {
        // assert!(self.owning_slab_id == peer.slabref.owning_slab_id, "apply_peer for dissimilar owning_slab_id peer"
        // );

        let peerlist = &mut self.0;
        {
            if let Some(my_peer) = peerlist.iter_mut().find(|p| p.slabref == peer.slabref) {
                if peer.status != my_peer.status {
                    // same slabref, so no need to apply the peer presence
                    my_peer.status = peer.status;
                    return true;
                } else {
                    return false;
                }
            }
        }

        peerlist.push(peer);
        true
    }
}

impl Deref for MemoPeerList {
    type Target = Vec<MemoPeer>;

    fn deref(&self) -> &Vec<MemoPeer> {
        &self.0
    }
}

#[derive(Clone, Debug)]
pub struct MemoPeer {
    pub slabref: SlabRef,
    pub status:  MemoPeeringStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MemoPeeringStatus {
    Resident,
    Participating,
    NonParticipating,
    Unknown,
}

pub type SlotId = u16;

#[derive(Clone, Debug, Serialize)]
pub struct RelationSet(pub HashMap<SlotId, Option<EntityId>>);

impl RelationSet {
    pub fn empty() -> Self {
        RelationSet(HashMap::new())
    }

    pub fn single(slot_id: SlotId, entity_id: EntityId) -> Self {
        let mut hashmap = HashMap::new();
        hashmap.insert(slot_id, Some(entity_id));
        RelationSet(hashmap)
    }

    pub fn insert(&mut self, slot_id: SlotId, entity_id: EntityId) {
        self.0.insert(slot_id, Some(entity_id));
    }

    pub fn concise_string(&self) -> String {
        self.0
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v.map(|x| x.to_string()).unwrap_or("NL".to_string())))
            .join(",")
    }
}

impl Deref for RelationSet {
    type Target = HashMap<SlotId, Option<EntityId>>;

    fn deref(&self) -> &HashMap<SlotId, Option<EntityId>> {
        &self.0
    }
}

// TODO: convert EdgeSet to use Vec<EdgeLink> - no need for a hashmap I think.
// Can use a sorted vec + binary search
#[derive(Clone, Debug)]
pub enum EdgeLink {
    Vacant { slot_id: SlotId },
    Occupied { slot_id: SlotId, head: Head },
}
// TODO: consider making this a Vec
#[derive(Clone, Debug, Default)]
pub struct EdgeSet(pub HashMap<SlotId, Head>);

impl EdgeSet {
    pub fn empty() -> Self {
        EdgeSet(HashMap::new())
    }

    pub fn single(slot_id: SlotId, head: Head) -> Self {
        let mut hashmap = HashMap::new();
        hashmap.insert(slot_id as SlotId, head);
        EdgeSet(hashmap)
    }

    pub fn insert(&mut self, slot_id: SlotId, head: Head) {
        self.0.insert(slot_id, head);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn concise_contents(&self) -> String {
        self.0
            .iter()
            .map(|(slot_id, head)| format!("{}:{}", slot_id, head.concise_contents()))
            .join(",")
    }
}

impl fmt::Display for EdgeSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.concise_contents())
    }
}
impl fmt::Display for RelationSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.concise_string())
    }
}

impl Deref for EdgeSet {
    type Target = HashMap<SlotId, Head>;

    fn deref(&self) -> &HashMap<SlotId, Head> {
        &self.0
    }
}
