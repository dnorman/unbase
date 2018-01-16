use slab::*;

use core;
use std::fmt;
use std::collections::HashMap;
use std::sync::{Arc,RwLock};

use memorefhead::*;
use context::Context;
use error::*;

use futures::{Stream,Sink,Future};
use futures::sync::mpsc::channel;

pub const SUBJECT_MAX_RELATIONS : usize = 256;
#[derive(Copy,Clone,Eq,PartialEq,Ord,PartialOrd,Hash,Debug,Serialize,Deserialize)]
pub enum SubjectType {
    IndexNode,
    Record
}
#[derive(Copy,Clone,Eq,PartialEq,Ord,PartialOrd,Hash,Debug,Serialize,Deserialize)]
pub struct SubjectId {
    pub id:    u64,
    pub stype: SubjectType,
}
impl <'a> core::cmp::PartialEq<&'a str> for SubjectId {
    fn eq (&self, other: &&'a str) -> bool {
        self.concise_string() == *other
    }
}

impl SubjectId {
    pub fn test(test_id: u64) -> Self{
        SubjectId{
            id:    test_id,
            stype: SubjectType::Record
        }
    }
    pub fn index_test(test_id: u64) -> Self{
        SubjectId{
            id:    test_id,
            stype: SubjectType::IndexNode
        }
    }
    pub fn concise_string (&self) -> String {
        use self::SubjectType::*;
        match self.stype {
            IndexNode => format!("I{}", self.id),
            Record    => format!("R{}", self.id)
        }
    }
}

impl fmt::Display for SubjectId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}-{}", self.stype, self.id)
    }
}

pub(crate) struct Subject {
    pub id:     SubjectId,
    pub (crate) head: Arc<RwLock<MemoRefHead>>, // Not sure how I feel about this being an Arc. Kind of a HACK.
    //pub (crate) rx: RwLock<Option<Box<Stream<Item=MemoRefHead, Error = ()>>>>
}

impl Subject {
    pub fn new (context: &Context, stype: SubjectType, vals: HashMap<String,String> ) -> Result<Self,WriteError> {

        let slab = &context.slab;
        let id = slab.generate_subject_id(stype);
        //println!("# Subject({}).new()",subject_id);

        let memoref = slab.new_memo_basic_noparent(
                Some(id),
                MemoBody::FullyMaterialized {v: vals, r: RelationSet::empty(), e: EdgeSet::empty(), t: stype.clone() }
            );
        let head = memoref.to_head();

        let subject = Subject{ id, head: Arc::new(RwLock::new(head.clone())) };

        //slab.subscribe_subject( &subject );

        subject.update_referents( context )?;

        Ok(subject)
    }
    /// Notify whomever needs to know that a new subject has been created
    fn update_referents (&self, context: &Context) -> Result<(),WriteError> {
        match self.id.stype {
            SubjectType::IndexNode => {
                let head = self.head.read().unwrap();
                context.apply_head( &*head )?;
            },
            SubjectType::Record    => {
                // TODO: Consider whether this should accept head instead of subject
                context.insert_into_root_index( self.id, &self )?;
            }
        }

        Ok(())
    }
    pub fn reconstitute (_context: &Context, head: MemoRefHead) -> Result<Subject,RetrieveError> {
        //println!("Subject.reconstitute({:?})", head);
        // Arguably we shouldn't ever be reconstituting a subject

        if let Some(subject_id) = head.subject_id(){
            let subject = Subject{
                id:   subject_id,
                head: Arc::new(RwLock::new(head))
            };

            // TODO3 - Should a resident subject be proactively updated? Or only when it's being observed?
            //context.slab.subscribe_subject( &subject );

            Ok(subject)

        }else{
            Err(RetrieveError::InvalidMemoRefHead)
        }
    }
    pub fn get_value ( &self, context: &Context, key: &str ) -> Result<Option<String>, RetrieveError> {
        //println!("# Subject({}).get_value({})",self.id,key);

        // TODO3: Consider updating index node ingress to mark relevant subjects as potentially dirty
        //        Use the lack of potential dirtyness to skip index traversal inside get_relevant_subject_head
        let chead = context.get_relevant_subject_head(self.id)?;
        //println!("\t\tGOT: {:?}", chead.memo_ids() );
        self.head.write().unwrap().apply( &chead, &context.slab )?;
        self.head.read().unwrap().project_value(&context.slab, key)
    }
    pub fn get_relation ( &self, context: &Context, key: RelationSlotId ) -> Result<Option<Subject>, RetrieveError> {
        //println!("# Subject({}).get_relation({})",self.id,key);
        self.head.write().unwrap().apply( &context.get_resident_subject_head(self.id), &context.slab )?;

        match self.head.read().unwrap().project_relation(&context.slab, key)? {
            Some(subject_id) => context.get_subject(subject_id),
            None             => Ok(None),
        }
    }
    pub fn get_edge ( &self, context: &Context, key: RelationSlotId ) -> Result<Option<Subject>, RetrieveError> {
        match self.get_edge_head(context,key)? {
            Some(head) => {
                Ok( Some( context.get_subject_with_head(head)? ) )
            },
            None => {
                Ok(None)
            }
        }
    }
    pub fn get_edge_head ( &self, context: &Context, key: RelationSlotId ) -> Result<Option<MemoRefHead>, RetrieveError> {
        //println!("# Subject({}).get_relation({})",self.id,key);
        self.head.write().unwrap().apply( &context.get_resident_subject_head(self.id), &context.slab )?;
        self.head.read().unwrap().project_edge(&context.slab, key)
    }

    pub fn set_value (&self, context: &Context, key: &str, value: &str) -> Result<bool,WriteError> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        let slab = &context.slab;
        {
            let mut head = self.head.write().unwrap();

            let memoref = slab.new_memo_basic(
                Some(self.id),
                head.clone(),
                MemoBody::Edit(vals)
            );

            head.apply_memoref(&memoref, &slab)?;
        }
        self.update_referents( context )?;

        Ok(true)
    }
    pub fn set_relation (&self, context: &Context, key: RelationSlotId, relation: &Self) -> Result<(),WriteError> {
        //println!("# Subject({}).set_relation({}, {})", &self.id, key, relation.id);
        let mut relationset = RelationSet::empty();
        relationset.insert( key, relation.id );

        let slab = &context.slab;
        {
            let mut head = self.head.write().unwrap();

            let memoref = slab.new_memo(
                Some(self.id),
                head.clone(),
                MemoBody::Relation(relationset)
            );

            head.apply_memoref(&memoref, &slab)?;
        };

        self.update_referents( context )
    }
    pub fn set_edge (&self, context: &Context, key: RelationSlotId, edge: &Self) -> Result<(),WriteError>{
        //println!("# Subject({}).set_edge({}, {})", &self.id, key, relation.id);
        let mut edgeset = EdgeSet::empty();
        edgeset.insert( key, edge.get_head() );

        let slab = &context.slab;
        {
            let mut head = self.head.write().unwrap();

            let memoref = slab.new_memo(
                Some(self.id),
                head.clone(),
                MemoBody::Edge(edgeset)
            );

            head.apply_memoref(&memoref, &slab)?;
        }
        
        self.update_referents( context )

    }
    // // TODO: get rid of apply_head and get_head in favor of Arc sharing heads with the context
    // pub fn apply_head (&self, context: &Context, new: &MemoRefHead){
    //     //println!("# Subject({}).apply_head({:?})", &self.id, new.memo_ids() );

    //     let slab = context.slab.clone(); // TODO: find a way to get rid of this clone

    //     //println!("# Record({}) calling apply_memoref", self.id);
    //     self.head.write().unwrap().apply(&new, &slab);
    // }
    pub fn get_head (&self) -> MemoRefHead {
        self.head.read().unwrap().clone()
    }
    // pub fn get_contextualized_head(&self, context: &Context) -> MemoRefHead {
    //     let mut head = self.head.read().unwrap().clone();
    //     head.apply( &context.get_resident_subject_head(self.id), &context.slab );
    //     head
    // }
    pub fn get_all_memo_ids ( &self, slab: &Slab ) -> Vec<MemoId> {
        //println!("# Subject({}).get_all_memo_ids()",self.id);
        self.get_head().causal_memo_iter( &slab ).map(|m| m.expect("Memo retrieval error. TODO: Update to use Result<..,RetrieveError>").id ).collect()
    }
    // pub fn is_fully_materialized (&self, context: &Context) -> bool {
    //     self.head.read().unwrap().is_fully_materialized(&context.slab)
    // }
    // pub fn fully_materialize (&self, _slab: &Slab) -> bool {
    //     unimplemented!();
    //     //self.shared.lock().unwrap().head.fully_materialize(slab)
    // }

    pub fn observe (&self, slab: &Slab) -> Box<Stream<Item=MemoRefHead, Error = ()>> {
        let (tx, rx) = channel::<MemoRefHead>(1);

        // TODO - figure out how to subscribe only once, such that one may create multiple observers for a single subject
        //        without duplication of effort
        let head = self.head.clone();
        let slab_clone = slab.clone();
        let rx2 = rx.map(move |mrh| {
            // TODO - make this more elegant, such that the initial MRH isn't redundantly applied to itself
            if let Err(_) =  head.write().unwrap().apply(&mrh,&slab_clone) {
                // TODO - Handle this somehow. (propagating the error to the observer seems odd)
            }

            mrh
        });

        // Send the initial MRH such that an observers gets the starting state
        tx.clone().send( self.head.read().unwrap().clone() ).wait().unwrap();
        
        slab.observe_subject( self.id, tx );
        Box::new(rx2)
    }
}

impl Clone for Subject {
    fn clone (&self) -> Subject {
        Self{
            id: self.id,
            head: self.head.clone()
        }
    }
}
impl Drop for Subject {
    fn drop (&mut self) {
        //println!("# Subject({}).drop", &self.id);
        // TODO: send a drop signal to the owning context via channel
        // self.drop_channel.send(self.id);
    }
}
impl fmt::Debug for Subject {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Subject")
            .field("subject_id", &self.id)
            .field("head", &self.head)
            .finish()
    }
}