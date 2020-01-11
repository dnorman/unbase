use util::serde::*;
//use network::TransportAddress;
use super::*;

/*
impl<'a> StatefulSerialize for &'a SlabPresence {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut sv = serializer.serialize_struct("SlabPresence", 3)?;
        sv.serialize_field("slab_id",  &self.slab_id)?;

        sv.serialize_field("address", match self.address {
            TransportAddress::Local   => helper.return_address,
            TransportAddress::UDP(_)  => &self.address,
            _ => return Err(SerError::custom("Address does not support serialization"))
        })?;
        sv.serialize_field("lifetime", &self.lifetime ) ?;
        sv.end()
    }
}
impl StatefulSerialize for SlabPresence {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut sv = serializer.serialize_struct("SlabPresence", 3)?;
        sv.serialize_field("slab_id",  &self.slab_id)?;

        sv.serialize_field("address", match self.address {
            TransportAddress::Local   => helper.return_address,
            TransportAddress::UDP(_)  => &self.address,
            _ => return Err(SerError::custom("Address does not support serialization"))
        })?;
        sv.serialize_field("lifetime", &self.lifetime ) ?;
        sv.end()
    }
}
*/

impl StatefulSerialize for SlabRef {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        // TODO: Should actually be a sequence of slab presences
        // to allow for slabs with multiple transports
        let mut seq = serializer.serialize_seq(Some(2))?;
        seq.serialize_element( &self.slab_id )?;
        seq.serialize_element( &self.get_presence_for_remote(helper.return_address) )?;
        //seq.serialize_element( &SerializeWrapper(&*(self.presence.read().unwrap()),helper) )?;
        seq.end()
    }
}


pub struct SlabRefSeed<'a> { pub dest_slab: &'a Slab }
impl<'a> DeserializeSeed for SlabRefSeed<'a> {
    type Value = SlabRef;

    fn deserialize<D> (self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {

        deserializer.deserialize_seq( self )
    }
}

impl<'a> Visitor for SlabRefSeed<'a> {
    type Value = SlabRef;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("SlabRef")
    }

    fn visit_seq<V> (self, mut visitor: V) -> Result<SlabRef, V::Error>
        where V: SeqVisitor
    {
        let slab_id: SlabId = match visitor.visit()? {
            Some(value) => value,
            None => {
                return Err(DeError::invalid_length(0, &self));
            }
        };
       let presence: Vec<SlabPresence> = match visitor.visit()? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(1, &self));
           }
       };

       let slabref = self.dest_slab.assert_slabref(slab_id, &presence); //.expect("slabref from slabrefseed presence");
       Ok( slabref )
    }
}
