use crate::slab::memoref_serde::*;
use crate::util::serde::*;
use ::serde::ser::*;
use ::serde::de::*;
use super::*;

impl StatefulSerialize for MemoRefHead {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        match *self {
            MemoRefHead::Null => {
                let sv = serializer.serialize_struct_variant("MemoRefHead", 0, "Null", 0)?;
                sv.end()
            },
            MemoRefHead::Anonymous{ref head} => {
                let mut sv = serializer.serialize_struct_variant("MemoRefHead", 1, "Anonymous", 1)?;
                sv.serialize_field("h", &SerializeWrapper(head, helper))?;
                sv.end()
            }
            MemoRefHead::Subject{ref subject_id, ref head} => {
                let mut sv = serializer.serialize_struct_variant("MemoRefHead", 2, "Subject", 3)?;
                sv.serialize_field("s", &subject_id)?;
                sv.serialize_field("h", &SerializeWrapper(&head,helper))?;
                sv.end()
            }
        }
    }
}


pub struct MemoRefHeadSeed<'a> { pub dest_slab: &'a Slab, pub origin_slabref: &'a SlabRef }

#[derive(Deserialize)]
enum MRHVariant{
    Null,
    Anonymous,
    Subject
}


impl<'de,'a> DeserializeSeed<'de> for MemoRefHeadSeed<'de> {
    type Value = MemoRefHead;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        const MRH_VARIANTS: &'static [&'static str] = &[
            "Null",
            "Anonymous",
            "Subject"
        ];

        deserializer.deserialize_enum("MemoRefHead", MRH_VARIANTS, self)
    }
}

impl<'de> Visitor<'de> for MemoRefHeadSeed<'de> {
    type Value = MemoRefHead;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("MemoRefHead")
    }
    fn visit_enum<A>(self, data: A) -> Result<MemoRefHead, A::Error>
        where A: EnumAccess<'de>
    {

        let foo = match data.variant()? {
            (MRHVariant::Null,       variant) => variant.newtype_variant_seed(MRHNullSeed{}),
            (MRHVariant::Anonymous,  variant) => variant.newtype_variant_seed(MRHAnonymousSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }),
            (MRHVariant::Subject,    variant) => variant.newtype_variant_seed(MRHSubjectSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref })
        };
        
        foo
    }
}

struct MRHNullSeed{}

impl<'de, 'a> DeserializeSeed<'de> for MRHNullSeed {
    type Value = MemoRefHead;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for MRHNullSeed {
    type Value = MemoRefHead;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoRefHead::Null")
    }
    fn visit_map<A>(self, _map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>,
    {
        Ok(MemoRefHead::Null)
    }
}

struct MRHAnonymousSeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }

impl<'de,'a> DeserializeSeed<'de> for MRHAnonymousSeed<'de> {
    type Value = MemoRefHead;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for MRHAnonymousSeed<'de> {
    type Value = MemoRefHead;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoRefHead::Anonymous")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>,
    {
        let mut head : Option<Vec<MemoRef>> = None;
        while let Some(key) = map.next_key()? {
            match key {
                'h' => head  = Some(map.next_value_seed(VecSeed(MemoRefSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }))?),
                _   => {}
            }
        }

        if head.is_some() {
            Ok(MemoRefHead::Anonymous{ head: head.unwrap() })
        }else{
            Err(DeError::invalid_length(0, &self))
        }

    }
}


struct MRHSubjectSeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }
impl<'de,'a> DeserializeSeed<'de> for MRHSubjectSeed<'de> {
    type Value = MemoRefHead;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer<'de>
    {
        deserializer.deserialize_any(self)
    }
}
impl<'de> Visitor<'de> for MRHSubjectSeed<'de> {
    type Value = MemoRefHead;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoRefHead::Subject")
    }
    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
        where A: MapAccess<'de>,
    {
        let mut head : Option<Vec<MemoRef>> = None;
        let mut subject_id : Option<SubjectId> = None;
        while let Some(key) = map.next_key()? {
            match key {
                's' => subject_id = Some(map.next_value()?),
                'h' => head        = Some(map.next_value_seed(VecSeed(MemoRefSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }))?),
                _   => {}
            }
        }

        if head.is_some() && subject_id.is_some(){
            Ok(MemoRefHead::Subject{
                head: head.unwrap(),
                subject_id: subject_id.unwrap(),
            })
        }else{
            Err(DeError::invalid_length(0, &self))
        }

    }
}
