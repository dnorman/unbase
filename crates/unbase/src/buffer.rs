use std::collections::HashMap;

use crate::{
    head::Head,
    network::{
        SlabRef,
        TransportAddress,
    },
    slab::{
        EdgeSet,
        EntityId,
        Memo,
        MemoBody,
        MemoId,
        MemoInner,
        MemoPeeringStatus,
        MemoRef,
        RelationSet,
        SlabId,
        SlotId,
        TransportLiveness,
    },
};
use serde::{
    Deserialize,
    Serialize,
};
use std::sync::Arc;

pub trait BufferHelper {
    type EntityToken;
    type MemoToken;
    type SlabToken;
    fn from_entity_id(&self, entity_id: &EntityId) -> Self::EntityToken;
    fn from_memoref(&self, memo_id: &MemoRef) -> Self::MemoToken;
    fn from_slab_id(&self, slab_id: &SlabId) -> Self::SlabToken;
    fn to_entity_id(&self, entity_token: &Self::EntityToken) -> EntityId;
    fn to_memoref(&self, memo_token: &Self::MemoToken) -> MemoRef;
    fn to_slab_id(&self, slab_token: &Self::SlabToken) -> SlabId;
}

// Items ending in *Buf are intended to be directly serializable/storable
// Items ending in *Element are intended to be serializable/storable ONLY as a constituent of a *Buf

// TODO: investigate converting these into serde stateful {de}serializers
//   this may or may not be possible depending on whether there are async methods in here

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlabBuf<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    //    slab_id:  SlabId, // This is always a real SlabId
    presence: Vec<SlabPresenceBufElement<E, M>>,
}

impl<E, M> SlabBuf<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub fn from_slabref<H>(slabref: &SlabRef, helper: &H) -> SlabBuf<E, M>
        where H: BufferHelper<EntityToken = E, MemoToken = M>
    {
        SlabBuf { presence: slabref.channels
                                   .read()
                                   .unwrap()
                                   .iter()
                                   .map(|c| {
                                       SlabPresenceBufElement::<E, M> { address:      c.address,
                                                                        liveness:     c.liveness,
                                                                        latest_clock: HeadBufElement::<E, M>::Null, }
                                   })
                                   .collect(), }
    }

    pub fn to_slabref<H>(self, helper: &H) -> SlabRef
        where H: BufferHelper<EntityToken = E, MemoToken = M>
    {
        unimplemented!()
        //        SlabRef { presence: slabref.channels
        //            .read()
        //            .unwrap()
        //            .iter()
        //            .map(|c| {
        //                SlabPresenceBufElement::<E, M> { address:      c.address,
        //                    liveness:     c.liveness,
        //                    latest_clock: HeadBufElement::<E, M>::Null, }
        //            })
        //            .collect(), }
    }
}

/// Intentionally not including MemoId here, because it's based on this content
/// The storage engine may choose to calculate the hash as part of storage, or it may choose not to, depending on the circumstance

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoBuf<E, M, S>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    entity:  Option<E>, // use Offset for net version
    parents: HeadBufElement<E, M>,
    body:    MemoBodyBufElement<E, M, S>,
}

impl<E, M, S> MemoBuf<E, M, S>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    pub fn from_memo<H>(memo: Memo, helper: &H) -> Self
        where H: BufferHelper<EntityToken = E, MemoToken = M, SlabToken = S>
    {
        Self { entity:  memo.entity_id.map(|eid| helper.from_entity_id(&eid)),
               parents: HeadBufElement::from_head(memo.parents, helper),
               body:    MemoBodyBufElement::from_body(memo.body, helper), }
    }

    pub fn to_memo<H>(self, helper: &H, slabref: &SlabRef) -> Memo
        where H: BufferHelper<EntityToken = E, MemoToken = M, SlabToken = S>
    {
        Memo(Arc::new(MemoInner { entity_id:      self.entity.map(|e| helper.to_entity_id(&e)),
                                  owning_slabref: slabref.clone(),
                                  parents:        self.parents.to_head(helper, slabref),
                                  body:           self.body.to_body(helper), }))
    }
}

/// Directly Stored in slab::State
/// We can key this on MemoId in the lookup, and thus not have to store remote memos at all

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoPeersBuf<M, S>
    where M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    memo_id: M,                          // use Offset in net version
    peers:   Vec<MemoPeerBufElement<S>>, // use Vec<Offset> in net version
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlabPresenceBufElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub address:  TransportAddress,
    pub liveness: TransportLiveness,
    latest_clock: HeadBufElement<E, M>, // (receipt of latest presence message? or any message?)
}

// TODO - should we be peering Heads instead of Memos??
// This could be more efficient. How would this effect clock behavior?

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum HeadBufElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    Null,
    Entity { entity: E, head: Vec<M> },
    Anonymous { head: Vec<M> },
}

impl<E, M> HeadBufElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub fn from_head<H>(head: Head, helper: &H) -> Self
        where H: BufferHelper<EntityToken = E, MemoToken = M>
    {
        match head {
            Head::Null => HeadBufElement::Null,
            Head::Anonymous { ref head, .. } => {
                HeadBufElement::Anonymous { head: head.iter().map(|mr| helper.from_memoref(mr)).collect(), }
            },
            Head::Entity { ref head, ref entity_id, .. } => {
                HeadBufElement::Entity { entity: helper.from_entity_id(entity_id),
                                         head:   head.iter().map(|mr| helper.from_memoref(mr)).collect(), }
            },
        }
    }

    pub fn to_head<H>(self, helper: &H, owning_slabref: &SlabRef) -> Head
        where H: BufferHelper<EntityToken = E, MemoToken = M>
    {
        match self {
            HeadBufElement::Null => Head::Null,
            HeadBufElement::Anonymous { ref head, .. } => {
                Head::Anonymous { owning_slabref: owning_slabref.clone(),
                                  head:           head.iter().map(|mt| helper.to_memoref(mt)).collect(), }
            },
            HeadBufElement::Entity { ref head, ref entity, .. } => {
                Head::Entity { owning_slabref: owning_slabref.clone(),
                               entity_id:      helper.to_entity_id(entity),
                               head:           head.iter().map(|mt| helper.to_memoref(mt)).collect(), }
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoPeerBufElement<S>
    where S: Serialize + Deserialize
{
    slab_id: S, // use Offset in net version
    status:  MemoPeeringStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EdgeSetBufElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub v: HashMap<SlotId, HeadBufElement<M, E>>,
}

impl<E, M> EdgeSetBufElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub fn from_edgeset<H>(edgeset: &EdgeSet, helper: &H) -> EdgeSetBufElement<E, M>
        where H: BufferHelper<EntityToken = E, MemoToken = M>
    {
        unimplemented!()
    }

    pub fn to_edgeset<H>(self, helper: &H) -> EdgeSet
        where H: BufferHelper<EntityToken = E, MemoToken = M>
    {
        unimplemented!()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelationSetBufElement<E>
    where E: Serialize + Deserialize
{
    pub slots: HashMap<SlotId, Option<E>>,
}

impl<E> RelationSetBufElement<E> where E: Serialize + Deserialize
{
    pub fn from_relationset<H>(relationset: &RelationSet, helper: &H) -> RelationSetBufElement<E>
        where H: BufferHelper<EntityToken = E>
    {
        RelationSetBufElement { slots:
                                    relationset.0
                                               .iter()
                                               .map(|(slot_id, opt_e)| {
                                                   (*slot_id, opt_e.as_ref().map(|e| helper.from_entity_id(e)))
                                               })
                                               .collect(), }
    }

    pub fn to_relationset<H>(self, helper: &H) -> RelationSet
        where H: BufferHelper<EntityToken = E>
    {
        RelationSet(self.slots
                        .iter()
                        .map(|(slot_id, opt_e)| (*slot_id, opt_e.as_ref().map(|et| helper.to_entity_id(et))))
                        .collect())
    }
}

// TODO
// pub struct EdgeSetElement{}
// pub struct ValueSetElement{}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MemoBodyBufElement<E, M, S>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    // TODO: split out root_index_seed conveyance to another memobody type
    // TODO: needed for NetBuffer
    SlabPresence {
        p: SlabPresenceBufElement<E, M>,
        r: HeadBufElement<E, M>,
    },
    Peering(M, Vec<MemoPeerBufElement<S>>),
    MemoRequest(Vec<M>, Vec<S>),
    Relation(RelationSetBufElement<E>),
    Edge(EdgeSetBufElement<E, M>),
    Edit(EditBufElement),
    FullyMaterialized {
        v: HashMap<String, String>,
        e: EdgeSetBufElement<E, M>,
        r: RelationSetBufElement<E>,
    },
    PartiallyMaterialized {
        v: HashMap<String, String>,
        e: EdgeSetBufElement<E, M>,
        r: RelationSetBufElement<E>,
    }, // TODO convert v to ValueSetElement
}

impl<E, M, S> MemoBodyBufElement<E, M, S>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    pub fn from_body<H: BufferHelper + Sized>(body: MemoBody, helper: &H) -> Self
        where H: BufferHelper<EntityToken = E, MemoToken = M, SlabToken = S>
    {
        match body {
            MemoBody::SlabPresence { ref p, ref r } => {
                //                MemoBody::SlabPresence { p: p.clone(),
                //                    r: self.localize_head(r, from_slabref, true), }
                unimplemented!()
            },
            MemoBody::Relation(ref relationset) => {
                MemoBodyBufElement::Relation(RelationSetBufElement::from_relationset(relationset, helper))
            },
            MemoBody::Edge(ref edgeset) => MemoBodyBufElement::Edge(EdgeSetBufElement::from_edgeset(edgeset, helper)),
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
                unimplemented!()
                //                MemoBody::Peering(memo_id, entity_id, self.localize_peerlist(peerlist))
            },
            MemoBody::MemoRequest(ref memo_ids, ref slabref) => {
                unimplemented!()
                //                MemoBodyBufElement::MemoRequest(memo_ids.clone(), self.localize_slabref(slabref))
            },
        }
    }

    pub fn to_body<H: BufferHelper + Sized>(self, helper: &H) -> MemoBody
        where H: BufferHelper<EntityToken = E, MemoToken = M, SlabToken = S>
    {
        unimplemented!()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EditBufElement {
    edit: HashMap<String, String>,
}

#[cfg(test)]
mod test {
    use crate::{
        buffer::{
            EditBufElement,
            HeadBufElement,
            MemoBodyBufElement,
            MemoBuf,
            MemoPeerBufElement,
            MemoPeersBuf,
            SlabBuf,
            SlabPresenceBufElement,
        },
        network::TransportAddress,
        slab::{
            MemoPeeringStatus,
            TransportLiveness,
        },
    };

    use crate::slab::SlabId;
    use std::collections::HashMap;

    #[unbase_test_util::async_test]
    async fn basic() {
        let sb = SlabBuf::<u32, u32> { // slab_id:  SlabId::dummy(),
                                       presence: vec![SlabPresenceBufElement { address:      TransportAddress::Blackhole,
                                                                               liveness:     TransportLiveness::Available,
                                                                               latest_clock: HeadBufElement::Null, }], };

        let mb = MemoBuf::<u32, u32> { entity:  Some(1),
                                       parents: HeadBufElement::Null,
                                       body:    MemoBodyBufElement::Edit(EditBufElement { edit: HashMap::new() }), };

        let pb = MemoPeersBuf::<u32, u32> { memo_id: 2,
                                            peers:   vec![MemoPeerBufElement { slab_id: 0,
                                                                               status:  MemoPeeringStatus::Resident, }], };

        assert_eq!(&serde_json::to_string(&sb).unwrap(),
                   "{\"presence\":[{\"address\":\"Blackhole\",\"lifetime\":\"Ephmeral\",\"latest_clock\":\"Null\"}],\"\
                    latest_clock\":\"Null\"}");
        assert_eq!(&serde_json::to_string(&mb).unwrap(),
                   "{\"entity_id\":1,\"parents\":\"Null\",\"body\":{\"Edit\":{\"edit\":{}}}}");
        assert_eq!(&serde_json::to_string(&pb).unwrap(),
                   "{\"memo_id\":2,\"peers\":[{\"slab_id\":0,\"status\":\"Resident\"}]}");
    }
}
