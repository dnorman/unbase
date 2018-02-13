use network::TransportAddress;
use slab;
use slab::prelude::*;

use serde_json;
use util::serde::{SerializeHelper,SerializeWrapper};



// impl fmt::Debug for Packet {
//     fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
//         fmt.debug_struct("Packet")
//             .field("from_slab_id", &self.from_slab_id)
//             .field("to_slab_id", &self.to_slab_id)
//             .field("memo", &self.memo)
//             .field("peerlist", &self.peerlist)
//             .finish()
//     }
// }


/// Memo contents only - for storage and retrieval
pub struct MemoBuffer (
    pub Vec<u8>
);

/// Contents, peerstate, sender and receiver - for transfer between slabs
pub struct PacketBuffer (
    pub Vec<u8>
);
impl PacketBuffer {
    pub fn new( memo: MemoBuffer, peerset: MemoPeerSet, dest: SlabRef, from_slabref: SlabRef, return_address: &TransportAddress) -> Self {
        let packet = Packet {
            to_slab_id: dest.slab_id(),
            from_slabref: from_slabref,
            memobuffer:      MemoBuffer::from_memo(memo),
            peerstate:  peerstate,
        };
        let helper = SerializeHelper {
            return_address,
            dest_slab_id:   &packet.to_slab_id,
        };

        PacketBuffer(serde_json::to_vec( &SerializeWrapper(&packet, &helper) ).expect("serde_json::to_vec"))
    }
    pub fn deserialize_onto_slab(&self, from_address: &TransportAddress, dest_slab: &LocalSlabHandle) {
        let mut deserializer = serde_json::Deserializer::from_slice(&self.0);

        let packet_seed : PacketSeed = PacketSeed{
            net: &net,
            source_address: 
        };

        match packet_seed.deserialize(&mut deserializer) {
            Ok(()) => {
                // PacketSeed actually does everything
            },
            Err(e) =>{
                println!("DESERIALIZE ERROR {}", e);
            }
        }
    )
}

struct Packet {
    pub to_slab_id: slab::SlabId,
    pub from_slabref: SlabRef,
    pub memo: MemoBuffer,
    pub peerset: MemoPeerSet,
}


use super::*;
use super::super::*;

use slab::prelude::memo_serde::*;
use slab::prelude::memoref_serde::*;
use util::serde::DeserializeSeed;
use util::serde::*;

impl StatefulSerialize for Packet {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element( &self.from_slab_id )?;
        seq.serialize_element( &self.to_slab_id )?;
        seq.serialize_element( &SerializeWrapper( &self.peerstate, helper ) )?;
        seq.serialize_element( &SerializeWrapper( &self.memo, helper ) )?;
        seq.end()
    }
}

pub struct PacketSeed <'a>{
    pub net: &'a Network,
    pub source_address: TransportAddress
}

impl<'a> DeserializeSeed for PacketSeed<'a>{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_seq( self )
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
           }
       };
       let to_slab_id: SlabId = match visitor.visit()? {
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
           if let Some(slab) = self.net.get_slab_handle( to_slab_id ) {
               dest_slab = slab;
           }else{
               return Err(DeError::custom("Destination slab not found"));
           }

       }

       let presence = SlabPresence{
           slab_id: from_slab_id,
           address: self.source_address.clone(),
           lifetime: SlabAnticipatedLifetime::Unknown
       };

        let origin_slabref = dest_slab.slab_handle_from_presence(presence);

       // no need to return the memo here, as it's added to the slab
       let peers = match visitor.visit_seed(VecSeed(MemoPeerSeed{ dest_slab: &dest_slab }))? {
           Some(p) => p,
           None    => {
               return Err(DeError::invalid_length(2, &self));
           }
       };

       // no need to return the memo here, as it's added to the slab
       if let None = visitor.visit_seed( MemoSeed {
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
