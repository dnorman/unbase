mod core;

use context;
pub (crate) mod handle;


use self::core::*;
use self::handle::*;
use context::*;
use slab::*;
use memorefhead::MemoRefHead;

// use core;
// use handle;

pub type SubjectId     = u64;
pub struct Subject;
pub const SUBJECT_MAX_RELATIONS : usize = 256;

use std::sync::Arc;
use std::collections::HashMap;

impl Subject {
    pub fn new ( context: &ContextHandle, vals: HashMap<String, String>, is_index: bool ) -> Result<SubjectHandle,String> {
        let slab = &context.core.slab;
        let subject_id = slab.generate_subject_id();
        //println!("# Subject({}).new()",subject_id);

        let memoref = slab.new_memo_basic_noparent(
                Some(subject_id),
                MemoBody::FullyMaterialized {v: vals, r: RelationSlotSubjectHead(HashMap::new()) }
            );
        let head = memoref.to_head();

        let core = Arc::new(SubjectCore::new(subject_id, head, context));

        context.subscribe_subject( &core );

        // HACK HACK HACK - this should not be a flag on the subject, but something in the payload I think
        if !is_index {
            // NOTE: important that we do this after the subject.shared.lock is released
            context.insert_into_root_index( subject_id, &core );
        }

        let handle = SubjectHandle{
            core: core,
            context: context.core.clone()
        };

        Ok(handle)
    }
    pub fn reconstitute (context: &Arc<ContextCore>, head: MemoRefHead) -> Arc<SubjectCore> {
        //println!("Subject.reconstitute({:?})", head);

        let subject_id = head.first_subject_id().unwrap();

        let core = Arc::new(SubjectCore::new( subject_id, head, context ) );
        context.subscribe_subject( &core );

        core
    }
    pub fn new_blank ( context: &ContextHandle ) -> Result<SubjectHandle,String> {
        Self::new( context, HashMap::new(), false )
    }
    pub fn new_kv ( context: &ContextHandle, key: &str, value: &str ) -> Result<SubjectHandle,String> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        Self::new( context, vals, false )
    }
}