use super::*;
use network::TransportAddress;

/// SlabPresence represents the expected reachability of a given Slab
/// Including Transport address and anticipated lifetime
#[derive(Clone, Serialize, Deserialize)]
pub struct SlabPresence {
    pub slab_id: SlabId,
    pub address: TransportAddress,
    pub lifetime: SlabAnticipatedLifetime,
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
            .field("lifetime", &self.lifetime)
            .finish()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SlabAnticipatedLifetime {
    Ephmeral,
    Session,
    Long,
    VeryLong,
    Unknown,
}

#[derive(Clone,Debug)]
pub struct MemoPeerList(pub Vec<MemoPeer>);

impl MemoPeerList {
    pub fn new(list: Vec<MemoPeer>) -> Self {
        MemoPeerList(list)
    }
    pub fn clone(&self) -> Self {
        MemoPeerList(self.0.clone())
    }
    pub fn clone_for_slab(&self, to_slab: &Slab) -> Self {
        MemoPeerList(self.0
            .iter()
            .map(|p| {
                MemoPeer {
                    slabref: p.slabref.clone_for_slab(to_slab),
                    status: p.status.clone(),
                }
            })
            .collect())
    }
    pub fn slab_ids(&self) -> Vec<SlabId> {
        self.0.iter().map(|p| p.slabref.slab_id).collect()
    }
    pub fn apply_peer(&mut self, peer: MemoPeer) -> bool {
        // assert!(self.owning_slab_id == peer.slabref.owning_slab_id, "apply_peer for dissimilar owning_slab_id peer" );

        let mut peerlist = &mut self.0;
        {
            if let Some(my_peer) = peerlist.iter_mut()
                .find(|p| p.slabref.slab_id == peer.slabref.slab_id) {
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
    pub status: MemoPeeringStatus,
}

#[derive(Debug,Clone,PartialEq,Serialize,Deserialize)]
pub enum MemoPeeringStatus {
    Resident,
    Participating,
    NonParticipating,
    Unknown,
}

pub type RelationSlotId = u8;

// TODO: convert RelationSlotSubjectHead to Vec<RelationLink> - no need for a hashmap I think.
// Can use a sorted vec + binary search
#[derive(Clone, Debug)]
pub struct RelationSlotSubjectHead(pub HashMap<RelationSlotId, (SubjectId, MemoRefHead)>);

pub enum RelationLink{
    Vacant {
        slot_id:    RelationSlotId,
    },
    Occupied {
        slot_id:    RelationSlotId,
        subject_id: SubjectId,
        head:       MemoRefHead
    }
}

impl RelationSlotSubjectHead {
    pub fn clone_for_slab(&self, from_slabref: &SlabRef, to_slab: &Slab) -> Self {

        // HERE HERE HERE TODO
        // panic!("check here to make sure that peers are being properly constructed for the root_index_seed");
        let new = self.0
            .iter()
            .map(|(slot_id, &(subject_id, ref mrh))| {
                (*slot_id, (subject_id, mrh.clone_for_slab(from_slabref, to_slab, false)))
            })
            .collect();

        RelationSlotSubjectHead(new)
    }
    pub fn empty() -> Self {
        RelationSlotSubjectHead(HashMap::new())
    }
    pub fn single(slot_id: RelationSlotId, subject_id: SubjectId, head: MemoRefHead) -> Self {
        let mut hashmap = HashMap::new();
        hashmap.insert(slot_id, (subject_id, head));
        RelationSlotSubjectHead(hashmap)
    }
    pub fn insert(&mut self, slot_id: RelationSlotId, subject_id: SubjectId, head: MemoRefHead) {
        self.0.insert(slot_id, (subject_id, head));
    }
}

impl Deref for RelationSlotSubjectHead {
    type Target = HashMap<RelationSlotId, (SubjectId, MemoRefHead)>;
    fn deref(&self) -> &HashMap<RelationSlotId, (SubjectId, MemoRefHead)> {
        &self.0
    }
}
