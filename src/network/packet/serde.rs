use super::*;
use super::super::*;

use crate::slab::memo_serde::MemoSeed;
use crate::slab::memoref_serde::MemoPeerSeed;
use crate::util::serde::DeserializeSeed;
use crate::util::serde::*;

impl StatefulSerialize for Packet {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element( &self.from_slab_id )?;
        seq.serialize_element( &self.to_slab_id )?;
        seq.serialize_element( &SerializeWrapper( &self.peerlist, helper ) )?;
        seq.serialize_element( &SerializeWrapper( &self.memo, helper ) )?;
        seq.end()
    }
}

pub struct PacketSeed <'a>{
    pub net: &'a Network,
    pub source_address: TransportAddress
}

impl<'de, 'a> DeserializeSeed<'de> for PacketSeed<'de>{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_seq( self )
    }
}

impl<'de> Visitor<'de> for PacketSeed<'de> {
    type Value = ();
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct Packet")
    }
    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where  A: SeqAccess<'de>
    {
       let from_slab_id: SlabId = match seq.next_element()? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(0, &self));
           }
       };
       let to_slab_id: SlabId = match seq.next_element()? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(1, &self));
           }
       };


       let dest_slab;
       if to_slab_id == 0 {
           // Should this be multiple slabs somehow?
           // If so, we'd have to bifurcate the deserialization process
           if let Some(slab) = self.net.get_representative_slab() {
               dest_slab = slab
           }else{
               return Err(DeError::custom("Unable to pick_arbitrary_slab"));
           }
       }else{
           if let Some(slab) = self.net.get_slab( to_slab_id ) {
               dest_slab = slab;
           }else{
               return Err(DeError::custom("Destination slab not found"));
           }

       }

       let from_presence =  SlabPresence{
           slab_id: from_slab_id,
           address: self.source_address.clone(),
           lifetime: SlabAnticipatedLifetime::Unknown
       };

       let origin_slabref = dest_slab.slabref_from_presence(&from_presence).expect("slabref from presence");

       // no need to return the memo here, as it's added to the slab
       let peers = match seq.next_element_seed(VecSeed(MemoPeerSeed{ dest_slab: &dest_slab }))? {
           Some(p) => p,
           None    => {
               return Err(DeError::invalid_length(2, &self));
           }
       };

       // no need to return the memo here, as it's added to the slab
       if let None = seq.next_element_seed( MemoSeed {
           dest_slab: &dest_slab,
           origin_slabref: &origin_slabref,
           from_presence: from_presence,
           peerlist: MemoPeerList::new(peers)
       } )? {
            return Err(DeError::invalid_length(3, &self));
       };


       Ok(())
   }
}
