use std::fmt;
use std::collections::HashMap;
use std::sync::{Arc,RwLock,Weak};

use super::*;
use slab::*;
use memorefhead::*;
use context::Context;
use error::*;

use std::ops::Deref;

pub type SubjectField  = String;


#[derive(Clone)]
pub struct SubjectCore(Arc<SubjectCoreInner>);

impl Deref for SubjectCore {
    type Target = SubjectCoreInner;
    fn deref(&self) -> &SubjectCoreInner {
        &*self.0
    }
}

pub struct SubjectCoreInner {
    pub id:     SubjectId,
    pub (crate) head:       RwLock<MemoRefHead>
}

impl SubjectCore {
    pub fn new (context: &Context, stype: SubjectType, vals: HashMap<String,String> ) -> Self {

        let slab = &context.slab;
        let id = slab.generate_subject_id();
        //println!("# Subject({}).new()",subject_id);

        let memoref = slab.new_memo_basic_noparent(
                Some(id),
                MemoBody::FullyMaterialized {v: vals, r: RelationSlotSubjectHead(HashMap::new()), t: stype }
            );
        let head = memoref.to_head();

        let core = SubjectCore(Arc::new(SubjectCoreInner{ id, head: RwLock::new(head) }));

        context.subscribe_subject( &core );

        // HACK HACK HACK - this should not be a flag on the subject, but something in the payload I think
        if let SubjectType::Record = stype {
            // NOTE: important that we do this after the subject.shared.lock is released
            context.insert_into_root_index( id, &core );
        }

        SubjectCore(Arc::new(SubjectCoreInner{
            id: id,
            head: RwLock::new(head),
            // drop_channel: context.drop_channel.clone()
        }))
    }

    pub fn reconstitute (context: &Context, head: MemoRefHead) -> SubjectCore {
        //println!("Subject.reconstitute({:?})", head);

        let subject_id = head.first_subject_id().unwrap();

        let core = SubjectCore(Arc::new(SubjectCoreInner{
            id: subject_id,
            head: RwLock::new(head),
            // drop_channel: context.drop_channel.clone()
        }));

        context.subscribe_subject( &core );

        core
    }
    pub fn is_root_index () -> bool {
        unimplemented!();
    }
    pub fn get_value ( &self, context: &Context, key: &str ) -> Option<String> {
        //println!("# Subject({}).get_value({})",self.id,key);
        self.head.read().unwrap().project_value(context, key)
    }
    pub fn get_relation ( &self, context: &Context, key: RelationSlotId ) -> Result<SubjectCore, RetrieveError> {
        //println!("# Subject({}).get_relation({})",self.id,key);
        match self.head.read().unwrap().project_relation(context, key) {
            Ok(subject_id) => context.get_subject_core(subject_id),
            Err(e)         => Err(e)
        }
    }
    pub fn get_edge ( &self, context: &Context, key: RelationSlotId ) -> Result<SubjectCore, RetrieveError> {
        //println!("# Subject({}).get_relation({})",self.id,key);
        match self.head.read().unwrap().project_edge(context, key) {
            Ok((subject_id, head)) => {
                Ok( context.get_subject_with_head(subject_id,head)? )
            },
            Err(e)   => Err(e)

        }
    }
    pub fn set_value (&self, context: &Context, key: &str, value: &str) -> bool {
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

        context.apply_head( self.id,  &head );

        true
    }
    pub fn set_relation (&self,context: &Context, key: RelationSlotId, relation: &Self) {
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
        context.apply_head( self.id, &head );

    }
    // TODO: get rid of apply_head and get_head in favor of Arc sharing heads with the context
    pub fn apply_head (&self, context: &Context, new: &MemoRefHead){
        //println!("# Subject({}).apply_head({:?})", &self.id, new.memo_ids() );

        let slab = context.slab.clone(); // TODO: find a way to get rid of this clone

        //println!("# Record({}) calling apply_memoref", self.id);
        self.head.write().unwrap().apply(&new, &slab);
    }
    pub fn get_head (&self) -> MemoRefHead {
        self.head.read().unwrap().clone()
    }
    pub fn get_all_memo_ids ( &self, context: &Context ) -> Vec<MemoId> {
        //println!("# Subject({}).get_all_memo_ids()",self.id);
        let slab = context.slab.clone(); // TODO: find a way to get rid of this clone
        self.head.read().unwrap().causal_memo_iter( &slab ).map(|m| m.id).collect()
    }
    pub fn is_fully_materialized (&self, context: &Context) -> bool {
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