use std::collections::HashMap;

use crate::{
    network::TransportAddress,
    slab::{
        EntityId,
        MemoId,
        MemoPeeringStatus,
        SlabAnticipatedLifetime,
        SlabId,
        SlotId,
    },
};
use serde::{
    Deserialize,
    Serialize,
};

pub trait BufferHelper {
    type EntityToken;
    type MemoToken;
    type SlabToken;
    fn from_entity_id(&self, entity_id: &EntityId) -> Self::EntityToken;
    fn from_memo_id(&self, memo_id: &MemoId) -> Self::MemoToken;
    fn from_slab_id(&self, slab_id: &SlabId) -> Self::SlabToken;
}

// Items ending in *Buf are intended to be directly serializable/storable
// Items ending in *Element are intended to be serializable/storable ONLY as a constituent of a *Buf

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlabBuf<E, M, S>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    presence:     Vec<SlabPresenceBufElement<E, M, S>>,
    latest_clock: HeadBufElement<E, M>, /* latest clock reading received from this Slab
                                         * may have other non-presence stuff here, like stats, or context-specific
                                         * interpretations of the SlabPresence perhaps */
}

/// Intentionally not including MemoId here, because it's based on this content
/// The storage engine may choose to calculate the hash as part of storage, or it may choose not to, depending on the circumstance

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoBuf<E, M, S>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    entity_id: Option<E>, // use Offset for net version
    parents:   HeadBufElement<E, M>,
    body:      MemoBodyBufElement<E, M, S>,
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
pub struct SlabPresenceBufElement<E, M, S>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub slab:     S,
    pub address:  TransportAddress,
    pub lifetime: SlabAnticipatedLifetime,
    latest_clock: HeadBufElement<M, E>, // (receipt of latest presence message? or any message?)
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelationSetBufElement<E>
    where E: Serialize + Deserialize
{
    pub slots: HashMap<SlotId, Option<E>>,
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
        p: SlabPresenceBufElement<E, M, S>,
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
            SlabAnticipatedLifetime,
        },
    };

    use std::collections::HashMap;

    #[unbase_test_util::async_test]
    async fn basic() {
        let sb = SlabBuf::<u32, u32> { presence:
                                           vec![SlabPresenceBufElement::<u32, u32> { address:      TransportAddress::Blackhole,
                                                                                     lifetime:
                                                                                         SlabAnticipatedLifetime::Ephmeral,
                                                                                     latest_clock: HeadBufElement::Null, }],
                                       latest_clock: HeadBufElement::Null, };

        let mb = MemoBuf::<u32, u32> { entity_id: 1,
                                       parents:   HeadBufElement::<u32, u32>::Null,
                                       body:      MemoBodyBufElement::<u32, u32>::Edit(EditBufElement { edit: HashMap::new() }), };

        let pb = MemoPeersBuf::<u32, u32> { memo_id: 2,
                                            peers:   vec![MemoPeerBufElement::<u32> { slab_id: 0,
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
