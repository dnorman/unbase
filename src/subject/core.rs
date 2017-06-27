use std::fmt;
use std::collections::HashMap;
use std::sync::{Arc,RwLock,Weak};

use super::*;
use slab::*;
use memorefhead::*;
use context::ContextCore;
use error::*;

pub type SubjectField  = String;

pub struct SubjectCore {
    pub id:     SubjectId,
    head:       RwLock<MemoRefHead>
}

impl SubjectCore {
    pub fn new (id: SubjectId, head: MemoRefHead, context: &Arc<ContextCore>) -> Arc<Self> {
        Arc::new(SubjectCore{
            id: id,
            head: RwLock::new(head),
            // drop_channel: context.drop_channel.clone()
        })
    }
    pub fn is_root_index {
        unimplemented!();
    }
    pub fn get_value ( &self, context: &Arc<ContextCore>, key: &str ) -> Option<String> {
        //println!("# Subject({}).get_value({})",self.id,key);
        self.head.read().unwrap().project_value(context, key)
    }
    pub fn get_relation ( &self, context: &Arc<ContextCore>, key: RelationSlotId ) -> Result<SubjectHandle, RetrieveError> {
        //println!("# Subject({}).get_relation({})",self.id,key);
        match self.head.read().unwrap().project_relation(&context, key) {
            Ok((subject_id, head)) => {
                SubjectHandle{
                    context: context.clone(),
                    core: context.get_subject_with_head(subject_id,head)?
                }
            },
            Err(e)   => Err(e)

        }
    }
    pub fn set_value (&self, context: Arc<ContextCore>, key: &str, value: &str) -> bool {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        let slab = &context.slab;
        let mut head = self.head.write().unwrap();

        let memoref = slab.new_memo_basic(
            Some(self.id),
            head.clone(),
            MemoBody::Edit(vals)
        );

        head.apply_memoref(&memoref, &slab);
        context.apply_head( self.id,  &head, false );

        true
    }
    pub fn set_relation (&self,context: Arc<ContextCore>, key: RelationSlotId, relation: &Self) {
        //println!("# Subject({}).set_relation({}, {})", &self.id, key, relation.id);
        let mut memoref_map : HashMap<RelationSlotId, (SubjectId,MemoRefHead)> = HashMap::new();
        memoref_map.insert(key, (relation.id, relation.get_head().clone()) );

        let slab = &context.slab;
        let mut head = self.head.write().unwrap();

        let memoref = slab.new_memo(
            Some(self.id),
            head.clone(),
            MemoBody::Relation(RelationSlotSubjectHead(memoref_map))
        );

        head.apply_memoref(&memoref, &slab);
        context.apply_head( self.id, &head, false );

    }
    // TODO: get rid of apply_head and get_head in favor of Arc sharing heads with the context
    pub fn apply_head (&self, context: Arc<ContextCore>, new: &MemoRefHead){
        //println!("# Subject({}).apply_head({:?})", &self.id, new.memo_ids() );

        let slab = context.slab.clone(); // TODO: find a way to get rid of this clone

        //println!("# Record({}) calling apply_memoref", self.id);
        self.head.write().unwrap().apply(&new, &slab);
    }
    pub fn get_head (&self) -> MemoRefHead {
        self.head.read().unwrap().clone()
    }
    pub fn get_all_memo_ids ( &self, context: Arc<ContextCore> ) -> Vec<MemoId> {
        //println!("# Subject({}).get_all_memo_ids()",self.id);
        let slab = context.slab.clone(); // TODO: find a way to get rid of this clone
        self.head.read().unwrap().causal_memo_iter( &slab ).map(|m| m.id).collect()
    }
    pub fn is_fully_materialized (&self, context: Arc<ContextCore>) -> bool {
        self.head.read().unwrap().is_fully_materialized(&context.slab)
    }
    pub fn fully_materialize (&self, _slab: &Slab) -> bool {
        unimplemented!();
        //self.shared.lock().unwrap().head.fully_materialize(slab)
    }
}

impl Drop for SubjectCore {
    fn drop (&mut self) {
        //println!("# Subject({}).drop", &self.id);
        // TODO: send a drop signal to the owning context via channel
        // self.drop_channel.send(self.id);
    }
}
impl fmt::Debug for SubjectCore {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Subject")
            .field("subject_id", &self.id)
            .field("head", &self.head)
            .finish()
    }
}

impl WeakSubject {
    pub fn upgrade (&self) -> Option<Subject> {
        match self.0.upgrade() {
            Some(s) => Some( Subject(s) ),
            None    => None
        }
    }
}
