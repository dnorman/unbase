use subject::{Subject, SubjectType};
use context::Context;
use slab::*;
use error::*;
use std::fmt;
use std::collections::HashMap;

#[derive(Clone)]
pub struct SubjectHandle {
    pub core:    Subject,
    pub context: Context
}

impl SubjectHandle{
    pub fn new ( context: &Context, vals: HashMap<String, String> ) -> Result<SubjectHandle,String> {

        let core = Subject::new(&context, SubjectType::Record, vals );

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
    pub fn get_value ( &self, key: &str ) -> Option<String> {
        self.core.get_value(&self.context, key)
    }
    pub fn get_relation ( &self, key: RelationSlotId ) -> Result<Option<SubjectHandle>, RetrieveError> {

         match self.core.get_relation(&self.context, key)?{
             Some(rel_sub_core) => {
                 Ok(Some(SubjectHandle{
                    context: self.context.clone(),
                    core: rel_sub_core
                }))
             },
             None => Ok(None)
         }

        

    }
    pub fn set_value (&self, key: &str, value: &str) -> bool {
        self.core.set_value(&self.context, key, value)
    }
    pub fn set_relation (&self, key: RelationSlotId, relation: &Self) {
        self.core.set_relation(&self.context, key, &relation.core)
    }
    pub fn get_all_memo_ids ( &self ) -> Vec<MemoId> {
        self.core.get_all_memo_ids(&self.context)
    }
}


impl fmt::Debug for SubjectHandle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Subject")
            .field("subject_id", &self.core.id)
            .field("head", &self.core.head)
            .finish()
    }
}