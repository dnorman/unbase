use core::ops::Deref;
use std::collections::HashMap;

use slab::prelude::*;
use subject::SubjectId;
use memorefhead::MemoRefHead;

pub type RelationSlotId = u8;

#[derive(Clone, Debug, Serialize)]
pub struct RelationSet(pub HashMap<RelationSlotId, Option<SubjectId>>);

impl RelationSet {
    pub fn clone_for_slab(&self, _to_slab: &LocalSlabHandle) -> Self {

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
    pub fn clone_for_slab(&self, from_slab: &LocalSlabHandle, to_slab: &LocalSlabHandle) -> Self {
        let new = self.0
            .iter()
            .map(|(slot_id, mrh)| {
                (*slot_id, mrh.clone_for_slab(from_slab, to_slab, false))
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
    peerlist:       Vec<MemoPeerState>
}