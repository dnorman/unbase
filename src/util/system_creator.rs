use std::collections::HashMap;
use memorefhead::MemoRefHead;
use slab::prelude::*;
use subject::SubjectType;

pub struct SystemCreator;

impl SystemCreator {

    pub fn generate_root_index_seed( slab: &LocalSlabHandle ) -> MemoRefHead {

        let mut values = HashMap::new();
        values.insert("tier".to_string(),0.to_string());

        let memoref = slab.new_memo_basic_noparent(
            slab.generate_subject_id(SubjectType::IndexNode),
            MemoBody::FullyMaterialized {
                v: values,
                e: EdgeSet::empty(),
                t: SubjectType::IndexNode
            }
        );

        memoref.to_head()
    }

}
