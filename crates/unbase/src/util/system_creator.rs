use crate::slab::IndexInfo;
use std::collections::HashMap;

use crate::{
    head::Head,
    slab::{
        EdgeSet,
        EntityType,
        MemoBody,
        RelationSet,
        SlabHandle,
    },
};

pub struct SystemCreator;

impl SystemCreator {
    pub fn generate_root_index_seed(slab: &SlabHandle) -> Head {
        let etype = EntityType::IndexNode(IndexInfo { tier: 0 });
        let memoref = slab.new_memo(Some(slab.generate_entity_id(etype)),
                                    Head::Null,
                                    MemoBody::FullyMaterialized { v: HashMap::new(),
                                                                  r: RelationSet::empty(),
                                                                  e: EdgeSet::empty(),
                                                                  t: etype, });

        memoref.to_head()
    }
}
