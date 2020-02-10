use std::collections::HashMap;

use crate::{
    network::TransportAddress,
    slab::{
        MemoPeeringStatus,
        SlabAnticipatedLifetime,
        SlotId,
    },
};
use serde::{
    Deserialize,
    Serialize,
};

// Items ending in *Buf are intended to be directly serializable/storable
// Items ending in *Element are intended to be serializable/storable ONLY as a constituent of a *Buf

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlabBuf<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    presence:     Vec<SlabPresenceElement<E, M>>,
    latest_clock: HeadElement<E, M>, /* latest clock reading received from this Slab
                                      * may have other non-presence stuff here, like stats, or context-specific
                                      * interpretations of the SlabPresence perhaps */
}

/// Intentionally not including MemoId here, because it's based on this content
/// The storage engine may choose to calculate the hash as part of storage, or it may choose not to, depending on the circumstance

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoBuf<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    entity_id: E, // use Offset for net version
    parents:   HeadElement<E, M>,
    body:      MemoBodyElement<E, M>,
}

/// Directly Stored in slab::State
/// We can key this on MemoId in the lookup, and thus not have to store remote memos at all

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoPeersBuf<M, S>
    where M: Serialize + Deserialize,
          S: Serialize + Deserialize
{
    memo_id: M,                       // use Offset in net version
    peers:   Vec<MemoPeerElement<S>>, // use Vec<Offset> in net version
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SlabPresenceElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub address:  TransportAddress,
    pub lifetime: SlabAnticipatedLifetime,
    latest_clock: HeadElement<M, E>, // (receipt of latest presence message? or any message?)
}

// TODO - should we be peering Heads instead of Memos??
// This could be more efficient. How would this effect clock behavior?

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum HeadElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    Null,
    Entity { entity_id: E, head: Vec<M> },
    Anonymous { head: Vec<M> },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MemoPeerElement<S>
    where S: Serialize + Deserialize
{
    slab_id: S, // use Offset in net version
    status:  MemoPeeringStatus,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EdgeSetElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    pub v: HashMap<SlotId, HeadElement<M, E>>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RelationSetElement<E>
    where E: Serialize + Deserialize
{
    pub slots: HashMap<SlotId, Option<E>>,
}

// TODO
// pub struct EdgeSetElement{}
// pub struct ValueSetElement{}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum MemoBodyElement<E, M>
    where E: Serialize + Deserialize,
          M: Serialize + Deserialize
{
    // Arguably we shouldn't ever be storing a SlabPresence, Peering, or MemoRequest Memo to disk, so consider removing these
    // entirely    SlabPresence{ p: SlabPresenceBuffer, r: Vec<MemoRefOffset> }, // TODO: split out root_index_seed
    // conveyance to another memobody type    Peering(MemoRefOffset,Vec<MemoPeerStateBuffer>),
    //    MemoRequest(Vec<MemoRefOffset>,Vec<SlabRefOffset>)
    Relation(RelationSetElement<E>),
    Edge(EdgeSetElement<E, M>),
    Edit(EditElement),
    FullyMaterialized {
        v: HashMap<String, String>,
        e: EdgeSetElement<E, M>,
    },
    PartiallyMaterialized {
        v: HashMap<String, String>,
        e: EdgeSetElement<E, M>,
    }, // TODO convert v to ValueSetElement
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct EditElement {
    edit: HashMap<String, String>,
}

#[cfg(test)]
mod test {
    use crate::{
        buffer::{
            EditElement,
            HeadElement,
            MemoBodyElement,
            MemoBuf,
            MemoPeerElement,
            MemoPeersBuf,
            SlabBuf,
            SlabPresenceElement,
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
        let sb = SlabBuf::<u32, u32> { presence:     vec![SlabPresenceElement::<u32, u32> { address:
                                                                                                TransportAddress::Blackhole,
                                                                                            lifetime:
                                                                                                SlabAnticipatedLifetime::Ephmeral,
                                                                                            latest_clock: HeadElement::Null, }],
                                       latest_clock: HeadElement::Null, };

        let mb = MemoBuf::<u32, u32> { entity_id: 1,
                                       parents:   HeadElement::<u32, u32>::Null,
                                       body:      MemoBodyElement::<u32, u32>::Edit(EditElement { edit: HashMap::new() }), };

        let pb = MemoPeersBuf::<u32, u32> { memo_id: 2,
                                            peers:   vec![MemoPeerElement::<u32> { slab_id: 0,
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
