mod core;

use context;
pub (crate) mod handle;


pub (crate) use self::core::*;
pub (crate) use self::handle::*;
use context::*;
use slab::*;
use memorefhead::MemoRefHead;

// use core;
// use handle;

pub type SubjectId     = u64;

#[derive(Clone,PartialEq,Debug,Serialize,Deserialize)]
pub enum SubjectType {
    IndexNode,
    Record
}

pub struct Subject;
pub const SUBJECT_MAX_RELATIONS : usize = 256;

use std::sync::Arc;
use std::collections::HashMap;

impl Subject {
    pub fn new ( context: &Context, vals: HashMap<String, String> ) -> Result<SubjectHandle,String> {

        let core = SubjectCore::new(&context, SubjectType::Record, vals );

        let handle = SubjectHandle{
            core: core,
            context: context.clone()
        };

        Ok(handle)
    }
    pub fn new_blank ( context: &Context ) -> Result<SubjectHandle,String> {
        Self::new( context, HashMap::new() )
    }
    pub fn new_kv ( context: &Context, key: &str, value: &str ) -> Result<SubjectHandle,String> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        Self::new( context, vals )
    }
}