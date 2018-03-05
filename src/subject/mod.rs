use slab::prelude::*;

use core;
use std::fmt;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;

use memorefhead::*;
use context::Context;
use error::*;

use futures::{Stream,Future};
//use futures::unsync::mpsc;

pub const SUBJECT_MAX_RELATIONS : usize = 256;
#[derive(Copy,Clone,Eq,PartialEq,Ord,PartialOrd,Hash,Debug,Serialize,Deserialize)]
pub enum SubjectType {
    Anonymous,
    IndexNode,
    Record,
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
    pub fn anonymous () -> Self {
        SubjectId{
            id: 0,
            stype: SubjectType::Anonymous
        }
    }
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
            Record    => format!("R{}", self.id),
            Anonymous => "ANON".to_string(),
        }
    }
    pub fn is_some(&self) -> bool {
        if let SubjectType::Anonymous = self.stype {
            false
        }else{
            true
        }
    }
    pub fn is_anonymous(&self) -> bool {
        if let SubjectType::Anonymous = self.stype {
            true
        }else{
            false
        }
    }
    pub fn ok(self) -> Option<SubjectId> {
        if let SubjectType::Anonymous = self.stype {
            None
        }else{
            Some(self)
        }
    }
    pub fn ok_or<E>(&self, err: E) -> Result<&Self, E> {
        if let SubjectType::Anonymous = self.stype {
            Err(err)
        }else{
            Ok(self)
        }
    }
    pub fn unwrap(self) -> Self {
        if let SubjectType::Anonymous = self.stype {
            panic!("Subject is Anonymous")
        }else{
            self
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
    pub (crate) head: MemoRefHeadOuter,
    //pub (crate) rx: RwLock<Option<Box<Stream<Item=MemoRefHead, Error = ()>>>>
}

impl Subject {
    pub fn new (context: &Context, stype: SubjectType, vals: HashMap<String,String> ) -> Box<Future<Item=Self,Error=Error>> {

        let slab = &context.slab;
        let id = slab.generate_subject_id(stype);
        //println!("# Subject({}).new()",subject_id);

        // TODO: Get rid of this clone
        let context_dup = context.clone();

        Box::new(slab.new_memo_basic_noparent(
            id,
            MemoBody::FullyMaterialized {v: vals, e: EdgeSet::empty(), t: stype }
        ).and_then(move |memoref|{

            let head = memoref.to_head_outer();
            let subject = Subject{ id, head };
            subject.update_referents( context_dup )?;

            Ok(subject)
        }))

    }
    /// Notify whomever needs to know that a new subject has been created
    fn update_referents (&self, context: Context) -> Result<(),Error> {
        use self::SubjectType::*;
        match self.id.stype {
            IndexNode => {
                let head = self.head.0.borrow();
                context.apply_head( &*head )?;
            },
            Record    => {
                // TODO: Consider whether this should accept head instead of subject
                context.insert_into_root_index( self.id, self )?;
            }
            Anonymous => {
                //
            }
        }

        Ok(())
    }
    pub fn reconstitute (_context: &Context, head: MemoRefHead) -> Result<Subject,Error> {
        //println!("Subject.reconstitute({:?})", head);
        // Arguably we shouldn't ever be reconstituting a subject

        let subject_id = head.subject_id();
        if !subject_id.is_anonymous(){
            let subject = Subject{
                id:   subject_id,
                head: Rc::new(RefCell::new(head))
            };

            // TODO3 - Should a resident subject be proactively updated? Or only when it's being observed?
            //context.slab.subscribe_subject( &subject );

            Ok(subject)

        }else{
            Err(Error::RetrieveError(RetrieveError::InvalidMemoRefHead))
        }
    }
    pub fn get_value ( &self, context: &Context, key: &str ) -> Result<Option<String>, Error> {
        //println!("# Subject({}).get_value({})",self.id,key);

        // TODO3: Consider updating index node ingress to mark relevant subjects as potentially dirty
        //        Use the lack of potential dirtyness to skip index traversal inside get_relevant_subject_head
        let chead = context.get_relevant_subject_head(self.id)?;
        //println!("\t\tGOT: {:?}", chead.memo_ids() );
        self.head.borrow_mut().apply( &chead, &context.slab )?;
        self.head.borrow().project_value(&context.slab, key)
    }
    pub fn get_edge ( &self, context: &Context, key: RelationSlotId ) -> Result<Option<Subject>, Error> {
        match self.get_edge_head(context,key)? {
            Some(head) => {
                Ok( Some( context.get_subject_with_head(head)? ) )
            },
            None => {
                Ok(None)
            }
        }
    }
    pub fn get_edge_head ( &self, context: &Context, key: RelationSlotId ) -> Result<Option<MemoRefHead>, Error> {
        //println!("# Subject({}).get_relation({})",self.id,key);
        self.head.borrow_mut().apply( &context.get_resident_subject_head(self.id), &context.slab )?;
        self.head.borrow().project_edge(&context.slab, key)
    }

    pub fn set_value (&self, context: &Context, key: &str, value: &str) -> Result<bool,Error> {

        let mut vals: HashMap<String, String> = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        let head: Rc<RefCell<MemoRefHead>> = self.head.clone();

        // TODO: get rid of these clones
        let slab_copy: LocalSlabHandle = context.slab.clone();
        let self_copy: Subject = self.clone();
        let context_copy: Context = context.clone();

        context.slab.new_memo_basic(
            self.id,
            self.head.clone(),
            MemoBody::Edit(vals)
        ).and_then(move |memoref|{
            head.apply_memoref(memoref, slab_copy).and_then(move |did_apply|{
                if did_apply {
                    self_copy.update_referents(context_copy)?;
                };

                Ok(did_apply)
            })
        }).wait()
    }
    pub fn set_edge (&self, context: &Context, key: RelationSlotId, edge: &Self) -> Result<(),Error>{
        //println!("# Subject({}).set_edge({}, {})", &self.id, key, relation.id);
        let mut edgeset = EdgeSet::empty();
        edgeset.insert( key, edge.get_head() );

        let slab = &context.slab;
        {
            let mut head = self.head.borrow_mut();

            let memoref = slab.new_memo(
                self.id,
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
        self.head.borrow().clone()
    }
    // pub fn get_contextualized_head(&self, context: &Context) -> MemoRefHead {
    //     let mut head = self.head.read().unwrap().clone();
    //     head.apply( &context.get_resident_subject_head(self.id), &context.slab );
    //     head
    // }
    pub fn get_head_memorefs ( &self, _slab: &LocalSlabHandle ) -> Vec<MemoRef> {
        //println!("# Subject({}).get_all_memorefs()",self.id);
        self.get_head().to_vec()
        //.causal_memo_iter( &slab ).map(|m| m.expect("Memo retrieval error. TODO: Update to use Result<..,Error>") ).collect()
    }
    // pub fn is_fully_materialized (&self, context: &Context) -> bool {
    //     self.head.read().unwrap().is_fully_materialized(&context.slab)
    // }
    // pub fn fully_materialize (&self, _slab: &Slab) -> bool {
    //     unimplemented!();
    //     //self.shared.lock().unwrap().head.fully_materialize(slab)
    // }

    pub fn observe (&self, slab: &LocalSlabHandle) -> Box<Stream<Item=(), Error = Error>> {

        // TODO - figure out how to subscribe only once, such that one may create multiple observers for a single subject
        //        without duplication of effort
        // TODO - make this more elegant, such that the initial MRH isn't redundantly applied to itself

        let head: Rc<RefCell<MemoRefHead>> = self.head.clone();
        let slab: LocalSlabHandle = slab.clone();
        let stream = slab.observe_subject( self.id ).map(move |mrh|{
            head.borrow_mut().apply(&mrh, &slab).and_then(|applied| Ok(()) )
        });

        Box::new(stream)
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