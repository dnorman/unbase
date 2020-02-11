use crate::{
    head::Head,
    slab::{
        memoref_serde::*,
        EntityId,
        MemoRef,
        SlabHandle,
        SlabRef,
    },
    util::serde::{
        DeError,
        SerializeHelper,
        SerializeWrapper,
        StatefulSerialize,
        VecSeed,
    },
};
use serde::{
    de::*,
    ser::*,
};
use std::fmt;

impl StatefulSerialize for Head {
    fn serialize<S>(&self, serializer: S, helper: &SerializeHelper) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        match *self {
            Head::Null => {
                let sv = serializer.serialize_struct_variant("Head", 0, "Null", 0)?;
                sv.end()
            },
            Head::Anonymous { ref head, .. } => {
                let mut sv = serializer.serialize_struct_variant("Head", 1, "Anonymous", 1)?;
                sv.serialize_field("h", &SerializeWrapper(head, helper))?;
                sv.end()
            },
            Head::Entity { ref entity_id, ref head, .. } => {
                let mut sv = serializer.serialize_struct_variant("Head", 2, "Entity", 3)?;
                sv.serialize_field("s", &entity_id)?;
                sv.serialize_field("h", &SerializeWrapper(&head, helper))?;
                sv.end()
            },
        }
    }
}

pub struct HeadSeed<'a> {
    pub dest_slab:      &'a SlabHandle,
    pub origin_slabref: &'a SlabRef,
}

#[derive(Deserialize)]
enum HeadVariant {
    Null,
    Anonymous,
    Entity,
}

impl<'a> DeserializeSeed for HeadSeed<'a> {
    type Value = Head;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        const HEAD_VARIANTS: &'static [&'static str] = &["Null", "Anonymous", "Entity"];

        deserializer.deserialize_enum("Head", HEAD_VARIANTS, self)
    }
}

impl<'a> Visitor for HeadSeed<'a> {
    type Value = Head;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Head")
    }

    fn visit_enum<V>(self, visitor: V) -> Result<Head, V::Error>
        where V: EnumVisitor
    {
        let foo = match visitor.visit_variant()? {
            (HeadVariant::Null, variant) => variant.visit_newtype_seed(HeadNullSeed {}),
            (HeadVariant::Anonymous, variant) => {
                variant.visit_newtype_seed(HeadAnonymousSeed { dest_slab:      self.dest_slab,
                                                               origin_slabref: self.origin_slabref, })
            },
            (HeadVariant::Entity, variant) => {
                variant.visit_newtype_seed(HeadEntitySeed { dest_slab:      self.dest_slab,
                                                            origin_slabref: self.origin_slabref, })
            },
        };

        foo
    }
}

struct HeadNullSeed {}

impl<'a> DeserializeSeed for HeadNullSeed {
    type Value = Head;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize(self)
    }
}
impl<'a> Visitor for HeadNullSeed {
    type Value = Head;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Head::Null")
    }

    fn visit_map<Visitor>(self, _visitor: Visitor) -> Result<Self::Value, Visitor::Error>
        where Visitor: MapVisitor
    {
        Ok(Head::Null)
    }
}

struct HeadAnonymousSeed<'a> {
    dest_slab:      &'a SlabHandle,
    origin_slabref: &'a SlabRef,
}

impl<'a> DeserializeSeed for HeadAnonymousSeed<'a> {
    type Value = Head;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize(self)
    }
}
impl<'a> Visitor for HeadAnonymousSeed<'a> {
    type Value = Head;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Head::Anonymous")
    }

    fn visit_map<Visitor>(self, mut visitor: Visitor) -> Result<Self::Value, Visitor::Error>
        where Visitor: MapVisitor
    {
        let mut head: Option<Vec<MemoRef>> = None;
        while let Some(key) = visitor.visit_key()? {
            match key {
                'h' => {
                    head = Some(visitor.visit_value_seed(VecSeed(MemoRefSeed { dest_slab:      self.dest_slab,
                                                                               origin_slabref: self.origin_slabref, }))?)
                },
                _ => {},
            }
        }

        if head.is_some() {
            Ok(Head::Anonymous { owning_slabref: self.dest_slab.my_ref.clone(),
                                 head:           head.unwrap(), })
        } else {
            Err(DeError::invalid_length(0, &self))
        }
    }
}

struct HeadEntitySeed<'a> {
    dest_slab:      &'a SlabHandle,
    origin_slabref: &'a SlabRef,
}
impl<'a> DeserializeSeed for HeadEntitySeed<'a> {
    type Value = Head;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where D: Deserializer
    {
        deserializer.deserialize(self)
    }
}
impl<'a> Visitor for HeadEntitySeed<'a> {
    type Value = Head;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("Head::Entity")
    }

    fn visit_map<Visitor>(self, mut visitor: Visitor) -> Result<Self::Value, Visitor::Error>
        where Visitor: MapVisitor
    {
        let mut head: Option<Vec<MemoRef>> = None;
        let mut entity_id: Option<EntityId> = None;
        while let Some(key) = visitor.visit_key()? {
            match key {
                's' => entity_id = Some(visitor.visit_value()?),
                'h' => {
                    head = Some(visitor.visit_value_seed(VecSeed(MemoRefSeed { dest_slab:      self.dest_slab,
                                                                               origin_slabref: self.origin_slabref, }))?)
                },
                _ => {},
            }
        }

        if head.is_some() && entity_id.is_some() {
            Ok(Head::Entity { owning_slabref: self.dest_slab.my_ref.clone(),
                              head:           head.unwrap(),
                              entity_id:      entity_id.unwrap(), })
        } else {
            Err(DeError::invalid_length(0, &self))
        }
    }
}
