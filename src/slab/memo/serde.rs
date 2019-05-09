use super::*;
use crate::memorefhead::serde::*;
use super::memoref::serde::MemoPeerSeed;

use crate::slab::slabref::serde::SlabRefSeed;
use crate::util::serde::*;

use std::fmt;
use ::serde::*;
use ::serde::ser::*;
use ::serde::de::*;

pub struct MemoBodySeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef }
#[derive(Clone)]
pub struct MBMemoRequestSeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }
struct MBSlabPresenceSeed <'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }
struct MBFullyMaterializedSeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }
// TODO convert this to a non-seed deserializer
struct MBPeeringSeed<'a> { dest_slab: &'a Slab }

impl StatefulSerialize for Memo {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_seq(Some(4))?;
        seq.serialize_element( &self.id )?;
        seq.serialize_element( &self.subject_id )?;
        seq.serialize_element( &SerializeWrapper( &self.body, helper ) )?;
        seq.serialize_element( &SerializeWrapper( &self.parents, helper ) )?;
        seq.end()
    }
}

impl StatefulSerialize for MemoBody {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        use super::MemoBody::*;
        match *self {
            SlabPresence{ ref p, ref r } =>{
                let mut sv = serializer.serialize_struct_variant("MemoBody", 0, "SlabPresence", 2)?;
                sv.serialize_field("p", &p)?;
                sv.serialize_field("r", &SerializeWrapper(r, helper))?;
                sv.end()
            }
            Relation(ref rel_set) => {
                //let mut sv = serializer.serialize_struct_variant("MemoBody", 1, "Relation", 1)?;
                //sv.serialize_field("r", &SerializeWrapper(rhm, helper))?;
                //sv.end()
                serializer.serialize_newtype_variant("MemoBody", 1, "Relation", &SerializeWrapper(&rel_set, helper) )
            },
            Edge(ref edge_set) => {
                //let mut sv = serializer.serialize_struct_variant("MemoBody", 1, "Relation", 1)?;
                //sv.serialize_field("r", &SerializeWrapper(rhm, helper))?;
                //sv.end()
                serializer.serialize_newtype_variant("MemoBody", 2, "Edge", &SerializeWrapper(&edge_set.0, helper) )
            },
            Edit(ref e) => {
                //let mut sv = serializer.serialize_struct_variant("MemoBody", 2, "Edit", 1)?;
                //sv.serialize_field("e", e )?;
                //sv.end()
                serializer.serialize_newtype_variant("MemoBody", 3, "Edit", &e )

            },
            FullyMaterialized{ ref v,  ref r, ref e, ref t }  => {
                let mut sv = serializer.serialize_struct_variant("MemoBody", 4, "FullyMaterialized", 3)?;
                sv.serialize_field("r", &SerializeWrapper(&r, helper))?;
                sv.serialize_field("e", &SerializeWrapper(&e.0, helper))?;
                sv.serialize_field("v", v)?;
                sv.serialize_field("t", t)?;
                sv.end()
            },
            PartiallyMaterialized{ ref v, ref r, ref e, ref t }  => {
                let mut sv = serializer.serialize_struct_variant("MemoBody", 5, "PartiallyMaterialized", 2)?;
                sv.serialize_field("r", &SerializeWrapper(&r, helper))?;
                sv.serialize_field("e", &SerializeWrapper(&e.0, helper))?;
                sv.serialize_field("v", v)?;
                sv.serialize_field("t", t)?;
                sv.end()
            },
            Peering( ref memo_id, ref subject_id, ref peerlist ) =>{
                let mut sv = serializer.serialize_struct_variant("MemoBody", 6, "Peering", 3)?;
                sv.serialize_field("i", memo_id )?;
                sv.serialize_field("j", subject_id )?;
                sv.serialize_field("l", &SerializeWrapper(peerlist,helper) )?;
                sv.end()
            }
            MemoRequest( ref memo_ids, ref slabref ) =>{
                let mut sv = serializer.serialize_struct_variant("MemoBody", 7, "MemoRequest", 2)?;
                sv.serialize_field("i", memo_ids )?;
                sv.serialize_field("s", &SerializeWrapper(slabref, helper))?;
                sv.end()
            }
        }

    }
}

impl <'a> StatefulSerialize for &'a RelationSet {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let _ = helper;
        serializer.serialize_newtype_struct("RelationSet",&self.0)
    }
}

impl StatefulSerialize for (SubjectId,MemoRefHead) {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut seq = serializer.serialize_tuple(2)?;
        seq.serialize_element( &self.0 )?;
        seq.serialize_element( &SerializeWrapper( &self.1, helper ) )?;
        seq.end()
    }
}

pub struct MemoSeed<'a> {
    pub dest_slab: &'a Slab,
    pub origin_slabref: &'a SlabRef,
    pub from_presence: SlabPresence,
    pub peerlist: MemoPeerList
}

impl<'de, 'a> DeserializeSeed<'de> for MemoSeed<'de> {
    type Value = ();
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for MemoSeed<'de> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("struct Memo")
    }

    fn visit_seq<V> (self, mut seq: V) -> Result<Self::Value, V::Error>
        where V: SeqAccess<'de>
    {
        let id: MemoId = match seq.next_element()? {
            Some(value) => value,
            None => {
               return Err(DeError::invalid_length(0, &self));
            }
       };
       let subject_id: Option<SubjectId> = match seq.next_element()? {
            Some(value) => value,
            None => {
               return Err(DeError::invalid_length(1, &self));
            }
       };
       let body: MemoBody = match seq.next_element_seed(MemoBodySeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref })? {
            Some(value) => value,
            None => {
               return Err(DeError::invalid_length(2, &self));
            }
       };

       let parents: MemoRefHead = match seq.next_element_seed(MemoRefHeadSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref })? {
           Some(value) => value,
           None => {
               return Err(DeError::invalid_length(3, &self));
           }
       };
        //println!("SERDE calling reconstitute_memo");
        let _memo = self.dest_slab.reconstitute_memo(id, subject_id, parents, body, self.origin_slabref, &self.peerlist ).0;

        Ok(())
    }
}

#[derive(Deserialize)]
enum MBVariant {
    SlabPresence,
    Relation,
    Edge,
    Edit,
    FullyMaterialized,
    PartiallyMaterialized,
    Peering,
    MemoRequest
}

impl<'de, 'a> DeserializeSeed<'de> for MemoBodySeed<'de> {
    type Value = MemoBody;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {

        const MEMOBODY_VARIANTS: &'static [&'static str] = &[
            "SlabPresence",
            "Relation",
            "Edge",
            "Edit",
            "FullyMaterialized",
            "PartiallyMaterialized",
            "Peering",
            "MemoRequest"
        ];

        deserializer.deserialize_enum("MemoBody", MEMOBODY_VARIANTS, self)
    }
}

impl<'de> Visitor<'de> for MemoBodySeed<'de> {
    type Value = MemoBody;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("MemoBody")
    }
    fn visit_enum<A>(self, data: A) -> Result<MemoBody, A::Error>
        where A: EnumAccess<'de>
    {

        match data.variant()? {
            (MBVariant::SlabPresence,      variant) => variant.newtype_variant_seed(MBSlabPresenceSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }),
            (MBVariant::Relation,          variant) => variant.newtype_variant_seed(RelationSetSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }).map(MemoBody::Relation),
            (MBVariant::Edge,              variant) => variant.newtype_variant_seed(EdgeSetSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }).map(MemoBody::Edge),
            (MBVariant::Edit,              variant) => variant.newtype_variant().map(MemoBody::Edit),
            (MBVariant::FullyMaterialized, variant) => variant.newtype_variant_seed(MBFullyMaterializedSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }),
        //  (MBVariant::PartiallyMaterialized, variant) => variant.newtype_variant_seed().map(MemoBody::PartiallyMaterialized),
            (MBVariant::Peering,           variant) => variant.newtype_variant_seed(MBPeeringSeed{ dest_slab: self.dest_slab }),
            (MBVariant::MemoRequest,       variant) => variant.newtype_variant_seed(MBMemoRequestSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }),
            _ => unimplemented!()

        }
    }
}
impl<'de, 'a> DeserializeSeed<'de> for MBMemoRequestSeed<'de> {
    type Value = MemoBody;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'de> Visitor<'de> for MBMemoRequestSeed<'de> {
    type Value = MemoBody;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("MemoBody::MemoRequest")
    }
    fn visit_map<V>(self, mut visitor: V) -> Result<Self::Value, V::Error>
       where V: MapAccess<'de>
    {
        let mut memo_ids : Option<Vec<MemoId>> = None;
        let mut slabref  : Option<SlabRef> = None;
        while let Some(key) = visitor.next_key()? {
            match key {
                'i' => memo_ids = visitor.next_value()?,
                's' => slabref  = Some(visitor.next_value_seed(SlabRefSeed{ dest_slab: self.dest_slab })?),
                _   => {}
            }
        }

        if memo_ids.is_some() && slabref.is_some() {

            Ok(MemoBody::MemoRequest( memo_ids.unwrap(), slabref.unwrap() ))
        }else{
            Err(DeError::invalid_length(0, &self))
        }
    }
}


struct RelationSetSeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }
impl<'de,'a> DeserializeSeed<'de> for RelationSetSeed<'de> {
    type Value = RelationSet;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}


impl<'de> Visitor<'de> for RelationSetSeed<'de> {
    type Value = RelationSet;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("RelationSet")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>,
    {
        let mut values : HashMap<RelationSlotId,Option<SubjectId>> = HashMap::new();

        let _ = self.dest_slab;
        let _ = self.origin_slabref;

        while let Some(slot) = map.next_key()? {
             let maybe_subject_id = map.next_value()?;
             values.insert(slot, maybe_subject_id);
        }

        Ok(RelationSet(values))
    }
}

struct EdgeSetSeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }
impl<'de,'a> DeserializeSeed<'de> for EdgeSetSeed<'de> {
    type Value = EdgeSet;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for EdgeSetSeed<'de> {
    type Value = EdgeSet;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("EdgeSet")
    }

    fn visit_map<Visitor>(self, mut visitor: Visitor) -> Result<Self::Value, Visitor::Error>
        where Visitor: MapAccess<'de>,
    {
        let mut values : HashMap<RelationSlotId,MemoRefHead> = HashMap::new();

        while let Some(slot) = visitor.next_key()? {
             let mrh = visitor.next_value_seed(MemoRefHeadSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref })?;
             values.insert(slot, mrh);
        }

        Ok(EdgeSet(values))
    }
}


impl<'de,'a> DeserializeSeed<'de> for MBSlabPresenceSeed<'de> {
    type Value = MemoBody;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for MBSlabPresenceSeed<'de> {
    type Value = MemoBody;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoBody::SlabPresence")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>,
    {

        let mut presence  = None;
        let mut root_index_seed : Option<MemoRefHead>   = None;
        while let Some(key) = map.next_key()? {
            match key {
                'p' => presence        = map.next_value()?,
                'r' => root_index_seed = Some(map.next_value_seed(MemoRefHeadSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref  })?),
                _   => {}
            }
        }
        if presence.is_some() &&root_index_seed.is_some() {
            Ok(MemoBody::SlabPresence{ p: presence.unwrap(), r: root_index_seed.unwrap() })
        }else{
            Err(DeError::invalid_length(0, &self))
        }
    }
}

impl<'de, 'a> DeserializeSeed<'de> for MBFullyMaterializedSeed<'de> {
    type Value = MemoBody;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for MBFullyMaterializedSeed<'de> {
    type Value = MemoBody;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoBody::FullyMaterialized")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>,
    {

        let mut relations = None;
        let mut edges = None;
        let mut values    = None;
        let mut stype     = None;
        while let Some(key) = map.next_key()? {
            match key {
                'r' => relations = Some(map.next_value_seed(RelationSetSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref })?),
                'e' => edges = Some(map.next_value_seed(EdgeSetSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref })?),
                'v' => values    = map.next_value()?,
                't' => stype     = map.next_value()?,
                _   => {}
            }
        }
        if relations.is_some() && values.is_some() && stype.is_some() {
            Ok(MemoBody::FullyMaterialized{ v: values.unwrap(), r: relations.unwrap(), e: edges.unwrap(), t: stype.unwrap() })
        }else{
            Err(DeError::invalid_length(0, &self))
        }
    }
}

impl<'de,'a> DeserializeSeed<'de> for MBPeeringSeed<'de> {
    type Value = MemoBody;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for MBPeeringSeed<'de> {
    type Value = MemoBody;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoBody::Peering")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>,
    {
        let mut memo_ids : Option<MemoId> = None;
        let mut subject_id: Option<Option<SubjectId>> = None;
        let mut peerlist   : Option<MemoPeerList> = None;
        while let Some(key) = map.next_key()? {
            match key {
                'i' => memo_ids  = map.next_value()?,
                'j' => subject_id = map.next_value()?,
                'l' => peerlist  = Some(MemoPeerList::new(map.next_value_seed(VecSeed(MemoPeerSeed{ dest_slab: self.dest_slab }))?)),
                _   => {}
            }
        }

        if memo_ids.is_some() && subject_id.is_some() && peerlist.is_some() {

            Ok(MemoBody::Peering(
                memo_ids.unwrap(),
                subject_id.unwrap(),
                peerlist.unwrap() ))
        }else{
            Err(DeError::invalid_length(0, &self))
        }

    }
}
