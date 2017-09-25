use slab::memoref_serde::*;
use util::serde::*;
use serde::ser::*;
use serde::de::*;
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
                let mut sv = serializer.serialize_struct_variant("MemoRefHead", 2, "Anonymous", 1)?;
                sv.serialize_field("h", &SerializeWrapper(head, helper))?;
                sv.end()
            }
            MemoRefHead::Subject{subject_id, ref head} => {
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
    None,
    Anonymous,
    Subject
}


impl<'a> DeserializeSeed for MemoRefHeadSeed<'a> {
    type Value = MemoRefHead;
    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        const MRH_VARIANTS: &'static [&'static str] = &[
            "None",
            "Anonymous",
            "Subject"
        ];
        deserializer.deserialize_enum("MemoRefHead", MRH_VARIANTS, self)
    }
}

impl<'a> Visitor for MemoRefHeadSeed<'a> {
    type Value = MemoRefHead;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
       formatter.write_str("MemoRefHead")
    }
    fn visit_enum<V>(self, visitor: V) -> Result<MemoRefHead, V::Error>
        where V: EnumVisitor
    {
        match try!(visitor.visit_variant()) {
            (MRHVariant::None,       _)       => Ok(MemoRefHead::Null),
            (MRHVariant::Anonymous,  variant) => variant.visit_newtype_seed(MRHAnonymousSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }),
            (MRHVariant::Subject,    variant) => variant.visit_newtype_seed(MRHSubjectSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref })
        }
    }
}


struct MRHAnonymousSeed<'a> { dest_slab: &'a Slab, origin_slabref: &'a SlabRef  }

impl<'a> DeserializeSeed for MRHAnonymousSeed<'a> {
    type Value = MemoRefHead;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize(self)
    }
}
impl<'a> Visitor for MRHAnonymousSeed<'a> {
    type Value = MemoRefHead;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoRefHead::Anonymous")
    }
    fn visit_map<Visitor>(self, mut visitor: Visitor) -> Result<Self::Value, Visitor::Error>
        where Visitor: MapVisitor,
    {
        let mut head : Option<Vec<MemoRef>> = None;
        while let Some(key) = visitor.visit_key()? {
            match key {
                'h' => head  = Some(visitor.visit_value_seed(VecSeed(MemoRefSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }))?),
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
impl<'a> DeserializeSeed for MRHSubjectSeed<'a> {
    type Value = MemoRefHead;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize(self)
    }
}
impl<'a> Visitor for MRHSubjectSeed<'a> {
    type Value = MemoRefHead;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("MemoRefHead::Subject")
    }
    fn visit_map<Visitor>(self, mut visitor: Visitor) -> Result<Self::Value, Visitor::Error>
        where Visitor: MapVisitor,
    {
        let mut head : Option<Vec<MemoRef>> = None;
        let mut subject_id : Option<SubjectId> = None;
        let mut stype : Option<SubjectType> = None;
        while let Some(key) = visitor.visit_key()? {
            match key {
                's' => subject_id = Some(visitor.visit_value()?),
                't' => stype      = Some(visitor.visit_value()?),
                'h' => head        = Some(visitor.visit_value_seed(VecSeed(MemoRefSeed{ dest_slab: self.dest_slab, origin_slabref: self.origin_slabref }))?),
                _   => {}
            }
        }

        if head.is_some() && subject_id.is_some() && stype.is_some(){
            Ok(MemoRefHead::Subject{
                head: head.unwrap(),
                subject_id: subject_id.unwrap(),
            })
        }else{
            Err(DeError::invalid_length(0, &self))
        }

    }
}
