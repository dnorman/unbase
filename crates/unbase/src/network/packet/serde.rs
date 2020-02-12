use super::{
    super::*,
    *,
};

use crate::{
    slab::{
        memo_serde::MemoSeed,
        memoref_serde::MemoPeerSeed,
    },
    util::serde::{
        DeserializeSeed,
        *,
    },
};

impl StatefulSerialize for SerdePacket {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element(&self.from_slab_id)?;
        seq.serialize_element(&self.to_slab_id)?;
        seq.serialize_element(&SerializeWrapper(&self.peerlist, helper))?;
        seq.serialize_element(&SerializeWrapper(&self.memo, helper))?;
        seq.end()
    }
}

pub struct PacketSeed<'a> {
    pub net:            &'a Network,
    pub source_address: TransportAddress,
}

impl<'a> DeserializeSeed for PacketSeed<'a> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'a> Visitor for PacketSeed<'a> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct Packet")
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
        where V: SeqVisitor
    {
        let from_slab_id: SlabId = match visitor.visit()? {
            Some(value) => value,
            None => {
                return Err(DeError::invalid_length(0, &self));
            },
        };
        let to_slab_id: Option<SlabId> = visitor.visit()?;

        let dest_slab;
        match to_slab_id {
            None => {
                // Should this be multiple slabs somehow?
                // If so, we'd have to bifurcate the deserialization process
                if let Some(slab) = self.net.get_representative_slab() {
                    dest_slab = slab
                } else {
                    return Err(DeError::custom("Unable to pick_arbitrary_slab"));
                }
            },
            Some(id) => {
                match self.net.get_slabhandle(&id) {
                    Ok(slab) => dest_slab = slab,
                    Err(e) => {
                        return Err(DeError::custom(format!("Destination slab not found: {:?}", e)));
                    },
                }
            },
        }

        let from_slabref = dest_slab.agent.get_slabref(&from_slab_id, None).unwrap();

        let from_presence = SlabPresence { slab_id:  from_slabref.slab_id,
                                           address:  self.source_address.clone(),
                                           liveness: TransportLiveness::Available, };

        let origin_slabref = (*dest_slab.agent).get_slabref(&from_presence.slab_id, Some(&[from_presence]))
                                               .expect("slabref from presence");

        // no need to return the memo here, as it's added to the slab
        let peers = match visitor.visit_seed(VecSeed(MemoPeerSeed { dest_slab: &dest_slab }))? {
            Some(p) => p,
            None => {
                return Err(DeError::invalid_length(2, &self));
            },
        };

        // no need to return the memo here, as it's added to the slab
        if let None = visitor.visit_seed(MemoSeed { dest_slab: &dest_slab,
                                                    origin_slabref: &origin_slabref,
                                                    from_presence,
                                                    peerlist: MemoPeerList::new(peers) })?
        {
            return Err(DeError::invalid_length(3, &self));
        };

        Ok(())
    }
}
