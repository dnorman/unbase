use std::fmt;
use core::ops::Deref;
use std::collections::HashMap;
use futures::sync::{mpsc,oneshot};

use error::*;
use slab::prelude::*;
use subject::SubjectId;
use memorefhead::MemoRefHead;

pub type LocalSlabRequester = mpsc::UnboundedSender<(LocalSlabRequest,oneshot::Sender<LocalSlabResponse>)>;

#[derive(Clone,Debug)]
pub struct MemoPeerList(pub Vec<MemoPeer>);

impl MemoPeerList {
    pub fn new(list: Vec<MemoPeer>) -> Self {
        MemoPeerList(list)
    }
    pub fn clone(&self) -> Self {
        MemoPeerList(self.0.clone())
    }
    pub fn clone_for_slab(&self, to_slab: &LocalSlabHandle) -> Self {
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

        let peerlist = &mut self.0;
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

#[derive(Clone, Debug, Serialize)]
pub struct RelationSet(pub HashMap<RelationSlotId, Option<SubjectId>>);

impl RelationSet {
    pub fn clone_for_slab(&self, _from_slabref: &SlabRef, _to_slab: &LocalSlabHandle) -> Self {

        self.clone()
        // let new = self.0
        //     .iter()
        //     .map(|(slot_id, &subject_id)| {
        //         (*slot_id, subject_id)
        //     })
        //     .collect();

        // RelationSet(new)
    }
    pub fn empty() -> Self {
        RelationSet(HashMap::new())
    }
    pub fn single(slot_id: RelationSlotId, subject_id: SubjectId) -> Self {
        let mut hashmap = HashMap::new();
        hashmap.insert(slot_id, Some(subject_id));
        RelationSet(hashmap)
    }
    pub fn insert(&mut self, slot_id: RelationSlotId, subject_id: SubjectId) {
        self.0.insert(slot_id, Some(subject_id));
    }
}

impl Deref for RelationSet {
    type Target = HashMap<RelationSlotId, Option<SubjectId>>;
    fn deref(&self) -> &HashMap<RelationSlotId, Option<SubjectId>> {
        &self.0
    }
}


// TODO: convert EdgeSet to use Vec<EdgeLink> - no need for a hashmap I think.
// Can use a sorted vec + binary search
#[derive(Clone,Debug)]
pub enum EdgeLink{
    Vacant {
        slot_id:    RelationSlotId,
    },
    Occupied {
        slot_id:    RelationSlotId,
        head:       MemoRefHead
    }
}

#[derive(Clone, Debug, Default)]
pub struct EdgeSet (pub HashMap<RelationSlotId, MemoRefHead>);

impl EdgeSet {
    pub fn clone_for_slab(&self, from_slabref: &SlabRef, to_slab: &LocalSlabHandle) -> Self {
        let new = self.0
            .iter()
            .map(|(slot_id, mrh)| {
                (*slot_id, mrh.clone_for_slab(from_slabref, to_slab, false))
            })
            .collect();

        EdgeSet(new)
    }
    pub fn empty() -> Self {
        EdgeSet(HashMap::new())
    }
    pub fn single(slot_id: RelationSlotId, head: MemoRefHead) -> Self {
        let mut hashmap = HashMap::new();
        hashmap.insert(slot_id as RelationSlotId, head);
        EdgeSet(hashmap)
    }
    pub fn insert(&mut self, slot_id: RelationSlotId, head: MemoRefHead) {
        self.0.insert(slot_id, head);
    }
    pub fn len (&self) -> usize {
        self.0.len()
    }
}

impl Deref for EdgeSet {
    type Target = HashMap<RelationSlotId, MemoRefHead>;
    fn deref(&self) -> &HashMap<RelationSlotId, MemoRefHead> {
        &self.0
    }
}


pub enum LocalSlabRequest {
    ReceiveMemoWithPeerList{ memo: Memo, peerlist: MemoPeerList, from_slab: SlabId },
    RemotizeMemoIds{ memo_ids: Vec<MemoId> },
    PutSlabPresence { presence: SlabPresence },
    GetMemo { memo_id: MemoId },
    SendMemo { slab_id: SlabId, memoref: MemoRef }
}
pub enum LocalSlabResponse {
    ReceiveMemoWithPeerList( Result<(),Error> ),
    RemotizeMemoIds( Result<(),Error> ),
    PutSlabPresence( Result<(),Error> ),
    GetMemo( Result<Memo,Error> )
}

pub enum SlabSend {
    DeserializedMemo( DeserializedMemo ),
}

pub struct MemoRefFromSlab(SlabRef,MemoRef);

pub struct DeserializedMemo { 
    id:             MemoId,
    subject_id:     SubjectId,
    parents:        MemoRefHead,
    body:           MemoBody,
    origin_slabref: SlabRef,
    peerlist:       MemoPeerList
}