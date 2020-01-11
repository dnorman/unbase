use std::fmt;
use network::{TransportAddress};
use slab::SlabId;

use serde::ser::{Serialize};
pub use serde::ser::{Serializer,SerializeStruct,SerializeSeq,SerializeMap};
pub use serde::de::{Deserializer,DeserializeSeed,Visitor,SeqVisitor};

pub use serde::ser::Error as SerError;
pub use serde::de::Error as DeError;

pub struct SerializeHelper<'a> {
    pub dest_slab_id: &'a SlabId,
    pub return_address: &'a TransportAddress
}

pub struct SerializeWrapper<'a, T: 'a> (
    pub &'a T,
    pub &'a SerializeHelper<'a>
);

pub trait StatefulSerialize {
    fn serialize<S>(&self, serializer: S, state: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer;
}

impl<'a, T> Serialize for SerializeWrapper<'a, T>
    where T: StatefulSerialize
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        self.0.serialize(serializer, self.1)
    }
}

impl<T> StatefulSerialize for Vec<T>
      where T: StatefulSerialize
{
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for e in self.iter(){
            seq.serialize_element( &SerializeWrapper( e, helper ) )?;
        }
        seq.end()
    }
}
impl<'a, T> StatefulSerialize for &'a Vec<T>
      where T: StatefulSerialize
{
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for e in self.iter(){
            seq.serialize_element( &SerializeWrapper( e, helper ) )?;
        }
        seq.end()
    }
}
// whoops, looks like I need a macro for n-tuples?

use core::hash::{Hash, BuildHasher};
use std::collections::HashMap;

impl<K,V,H> StatefulSerialize for HashMap<K,V,H>
    where K: Serialize + Eq + Hash,
          V: StatefulSerialize,
          H: BuildHasher
{
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let iter = self.into_iter();
        let hint = match iter.size_hint() {
            (lo, Some(hi)) if lo == hi => Some(lo),
            _ => None,
        };
        let mut serializer = try!(serializer.serialize_map(hint));
        for (key, value) in iter {
            try!(serializer.serialize_entry(&key, &SerializeWrapper(value,helper)));
        }
        serializer.end()

    }
}

impl<T> StatefulSerialize for Option<T>
      where T: StatefulSerialize
{
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        match self {
            &Some(ref v) =>{
                serializer.serialize_some( &SerializeWrapper( v, helper ) )
            }
            &None => {
                serializer.serialize_none()
            }
        }
    }
}


pub struct VecSeed<S>(pub S);

impl<S> DeserializeSeed for VecSeed<S>
    where S: DeserializeSeed + Clone
{
    type Value = Vec<S::Value>;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_seq(self)
    }
}

impl<S> Visitor for VecSeed<S>
    where S: DeserializeSeed + Clone
{
    type Value = Vec<S::Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("sequence")
    }

    fn visit_seq<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
       where V: SeqVisitor
    {

        let mut out : Vec<S::Value> = Vec::new();

        while let Some(v) = visitor.visit_seed( self.0.clone() )? {
            out.push(v);
        };

        Ok(out)
    }
}
/// optional one.
pub struct OptionSeed<S>(pub S);

impl<S> Visitor for OptionSeed<S>
    where S: DeserializeSeed
{
    type Value = Option<S::Value>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("option")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
        where E: DeError
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        self.0.deserialize(deserializer).map(Some)
    }
}

impl<S> DeserializeSeed for OptionSeed<S>
    where S: DeserializeSeed
{
    type Value = Option<S::Value>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize_option(self)
    }
}
