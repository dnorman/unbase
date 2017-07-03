use super::*;
use super::core::*;
use context::Context;
use slab::*;
use error::*;

use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub struct SubjectHandle {
    pub core:    SubjectCore,
    pub context: Context
}

impl SubjectHandle{
    pub fn get_value ( &self, key: &str ) -> Option<String> {
        self.core.get_value(&self.context, key)
    }
    pub fn get_relation ( &self, key: RelationSlotId ) -> Result<SubjectHandle, RetrieveError> {
        let core = self.core.get_relation(&self.context, key)?;

        Ok(SubjectHandle{
            context: self.context.clone(),
            core: core
        })

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