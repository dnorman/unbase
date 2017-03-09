use std::fmt;
use std::collections::HashMap;
use memo::*;
use memoref::MemoRef;
use memorefhead::*;
use context::Context;
use error::*;
use slab::*;
use std::sync::{Arc,Mutex,Weak};

pub type SubjectId     = u64;
pub type SubjectField  = String;
pub const SUBJECT_MAX_RELATIONS : u16 = 256;

#[derive(Clone)]
pub struct Subject {
    pub id:  SubjectId,
    shared: Arc<Mutex<SubjectShared>>
}

pub struct WeakSubject {
    pub id:  SubjectId,
    shared: Weak<Mutex<SubjectShared>>
}

pub struct SubjectShared {
    id:      SubjectId,
    head:    MemoRefHead,
    context: Context,
}

impl Subject {
    pub fn new ( context: &Context, vals: HashMap<String, String>, is_index: bool ) -> Result<Subject,String> {

        let slab : &Slab = context.get_slab();
        let subject_id = slab.generate_subject_id();
        println!("# Subject({}).new()",subject_id);

        let shared = Arc::new(Mutex::new(SubjectShared{
            id: subject_id,
            head: MemoRefHead::new(),
            context: context.clone()
        }));

        let subject = Subject {
            id:      subject_id,
            shared:  shared
        };

        context.subscribe_subject( &subject );

        let memorefs = slab.put_memos(MemoOrigin::Local, vec![
            Memo::new_basic_noparent(
                slab.gen_memo_id(),
                subject_id,
                MemoBody::FullyMaterialized {v: vals, r: HashMap::new() } // TODO: accept relations
            )
        ], false);

        {
            let mut shared = subject.shared.lock().unwrap();
            shared.head.apply_memorefs(&memorefs, &slab);
            shared.context.subject_updated( &subject_id, &memorefs, &shared.head );


            // IMPORTANT: Need to wait to insert this into the index until _after_ the first memo
            // has been issued, sent to the slab, and added to the subject head via the subscription mechanism.

            // TODO: Decide if we want to redundantly add this memo to the head ( directly, and again through slab subscription )

            // HACK HACK HACK - this should not be a flag on the subject, but something in the payload I think
            if !is_index {
                context.insert_into_root_index( subject_id, &subject );
            }
        }
        Ok(subject)
    }
    pub fn reconstitute (context: &Context, head: MemoRefHead) -> Subject {

        let subject_id = head.get_first_subject_id( context.get_slab() ).unwrap();

        let shared = SubjectShared{
            id: subject_id,
            head: head,
            context: context.clone()
        };

        let subject = Subject {
            id:      subject_id,
            shared: Arc::new(Mutex::new(shared))
        };

        context.subscribe_subject( &subject );

        subject
    }
    pub fn new_kv ( context: &Context, key: &str, value: &str ) -> Result<Subject,String> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        Self::new( context, vals, false )
    }
    pub fn set_kv (&self, key: &str, value: &str) -> bool {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        let slab;
        let memos;
        {
            let shared = self.shared.lock().unwrap();
            slab = shared.context.get_slab().clone();

            memos = vec![
                Memo::new_basic(
                    slab.gen_memo_id(),
                    self.id,
                    shared.head.clone(),
                    MemoBody::Edit(vals)
                )
            ];
        }

        let memorefs : Vec<MemoRef> = slab.put_memos(MemoOrigin::Local, memos, false);

        let mut shared = self.shared.lock().unwrap();
        shared.head.apply_memorefs(&memorefs, &slab);
        shared.context.subject_updated( &self.id, &memorefs, &shared.head );

        true
    }
    pub fn get_value ( &self, key: &str ) -> Option<String> {
        println!("# Subject({}).get_value({})",self.id,key);

        //TODO: consider creating a consolidated projection routine for most/all uses
        for memo in self.memo_iter() {

            println!("# \t\\ Considering Memo {}", memo.id );
            if let Some((values, materialized)) = memo.get_values() {
                if let Some(v) = values.get(key) {
                    return Some(v.clone());
                }else if materialized {
                    return None; //end of the line here
                }
            }
        }
        None
    }
    pub fn set_relation (&self, key: u8, relation: Self) {
        println!("# Subject({}).set_relation({}, {})", &self.id, key, relation.id);
        let mut memoref_map : HashMap<u8, (SubjectId,MemoRefHead)> = HashMap::new();
        memoref_map.insert(key, (relation.id, relation.get_head().clone()) );

        let slab;
        let memos;
        {
            let shared = self.shared.lock().unwrap();
            slab = shared.context.get_slab().clone();

            memos = vec![
               Memo::new(
                   slab.gen_memo_id(), // TODO: lazy memo hash gen should eliminate this
                   self.id,
                   shared.head.clone(),
                   MemoBody::Relation(memoref_map)
               )
            ];
        }

        let memorefs = slab.put_memos( MemoOrigin::Local, memos, false );


        // TODO: determine conclusively whether it's possible for apply_memorefs
        //       to result in a retrieval that retults in a context addition that
        //       causes a deadlock
        let mut shared = self.shared.lock().unwrap();
        shared.head.apply_memorefs(&memorefs, &slab);
        shared.context.subject_updated( &self.id, &memorefs, &shared.head );

    }
    pub fn get_relation ( &self, key: u8 ) -> Result<Subject, RetrieveError> {
        println!("# Subject({}).get_relation({})",self.id,key);

        let shared = self.shared.lock().unwrap();
        let context = &shared.context;

        // TODO: Make error handling more robust

        for memo in shared.memo_iter() {

            if let Some((relations,materialized)) = memo.get_relations(){
                println!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}", memo.id, memo.get_parent_head(), relations );
                if let Some(r) = relations.get(&key) {
                    // BUG: the parent->child was formed prior to the revision of the child.
                    // TODO: Should be adding the new head memo to the query context
                    //       and superseding the referenced head due to its inclusion in the context

                    let foo = context.get_subject_with_head(r.0,r.1.clone());

                        return foo;
                }else if materialized {
                    println!("\n# \t\\ Not Found (materialized)" );
                    return Err(RetrieveError::NotFound);
                }
            }
        }

        println!("\n# \t\\ Not Found" );
        Err(RetrieveError::NotFound)
    }
    pub fn apply_head (&mut self, new: &MemoRefHead){
        println!("# Subject({}).apply_head({:?})", &self.id, new.memo_ids() );

        let mut shared = self.shared.lock().unwrap();
        let slab = shared.context.get_slab().clone(); // TODO: find a way to get rid of this clone

        println!("# Record({}) calling apply_memoref", self.id);
        shared.head.apply(&new, &slab);
    }
    pub fn get_head (&self) -> MemoRefHead {
        let shared = self.shared.lock().unwrap();
        shared.head.clone()
    }
    pub fn get_all_memo_ids ( &self ) -> Vec<MemoId> {
        println!("# Subject({}).get_all_memo_ids()",self.id);

        self.memo_iter().map(|m| m.id).collect()
    }
    fn memo_iter (&self) -> CausalMemoIter {
        self.shared.lock().unwrap().memo_iter()
    }
    pub fn weak (&self) -> WeakSubject {
        WeakSubject {
            id: self.id,
            shared: Arc::downgrade(&self.shared)
        }
    }
    pub fn is_fully_materialized (&self, slab: &Slab) -> bool {
        self.shared.lock().unwrap().head.is_fully_materialized(slab)
    }
    pub fn fully_materialize (&mut self, slab: &Slab) -> bool {
        self.shared.lock().unwrap().head.fully_materialize(slab)
    }
}

impl SubjectShared {
    pub fn memo_iter(&self) -> CausalMemoIter {
        self.head.causal_memo_iter(self.context.get_slab())
    }
}

impl Drop for SubjectShared {
    fn drop (&mut self) {
        println!("# Subject({}).drop", &self.id);
        self.context.unsubscribe_subject(self.id);
    }
}
impl fmt::Debug for SubjectShared {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {

        fmt.debug_struct("Subject")
            .field("subject_id", &self.id)
            .field("head", &self.head)
            .finish()
    }
}

impl fmt::Debug for Subject {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let shared = self.shared.lock().unwrap();

        fmt.debug_struct("Subject")
            .field("subject_id", &self.id)
            .field("head", &shared.head)
            .finish()
    }
}

impl WeakSubject {
    pub fn upgrade (&self) -> Option<Subject> {
        match self.shared.upgrade() {
            Some(s) => Some( Subject { id: self.id, shared: s } ),
            None    => None
        }
    }
}
