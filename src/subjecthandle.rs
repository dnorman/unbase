use subject::{Subject, SubjectId, SubjectType};
use context::Context;
use slab::prelude::*;
use error::*;
use std::fmt;
use std::collections::HashMap;
use futures::{Stream};

#[derive(Clone)]
pub struct SubjectHandle {
    pub id: SubjectId,
    pub (crate) subject: Subject,
    pub (crate) context: Context
}

impl SubjectHandle{
    pub fn new ( context: &Context, vals: HashMap<String, String> ) -> Result<SubjectHandle,Error> {

        let subject = Subject::new(context, SubjectType::Record, vals )?;

        let handle = SubjectHandle{
            id: subject.id,
            subject: subject,
            context: context.clone()
        };

        Ok(handle)
    }
    pub fn new_blank ( context: &Context ) -> Result<SubjectHandle,Error> {
        Self::new( context, HashMap::new() )
    }
    pub fn new_kv ( context: &Context, key: &str, value: &str ) -> Result<SubjectHandle,Error> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        Self::new( context, vals )
    }
    pub fn get_value ( &self, key: &str ) -> Option<String> {
        self.subject.get_value(&self.context, key).expect("Retrieval error. TODO: Convert to Result<..,Error>")
    }
    pub fn set_value (&self, key: &str, value: &str) -> Result<bool,Error> {
        self.subject.set_value(&self.context, key, value)
    }
    pub fn head_memo_ids (&self) -> Vec<MemoId> {
        self.subject.get_head().memo_ids()
    }
    pub fn get_head_memorefs ( &self ) -> Vec<MemoRef> {
        self.subject.get_head_memorefs(&self.context.slab)
    }
    pub fn observe (&self) -> Box<Stream<Item = (), Error = Error>> {
        self.subject.observe(&self.context.slab)
    }
}


impl fmt::Debug for SubjectHandle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Subject")
            .field("subject_id", &self.subject.id)
            .field("head", &self.subject.head)
            .finish()
    }
}