use context::*;
use error::*;
use subject::{Subject,SubjectType,SUBJECT_MAX_RELATIONS};
use subjecthandle::SubjectHandle;
use slab::*;
use memorefhead::MemoRefHead;

use error::RetrieveError;
use std::collections::HashMap;


pub struct IndexFixed {
    root: Subject,
    depth: u8
}

impl IndexFixed {
    pub fn new (context: &Context, depth: u8) -> Result<IndexFixed,WriteError> {
        Ok(Self {
            root: Subject::new( context, SubjectType::IndexNode, HashMap::new() )?,
            depth: depth
        })
    }
    pub fn new_from_memorefhead (context: &Context, depth: u8, memorefhead: MemoRefHead ) -> IndexFixed {
        Self {
            root: Subject::reconstitute( context, memorefhead ).unwrap(),
            depth: depth
        }
    }
    pub fn insert_subject_handle (&self, key: u64, subjecthandle: &SubjectHandle) -> Result<(),WriteError> {
        self.insert(&subjecthandle.context, key, &subjecthandle.subject)
    }
    pub (crate) fn insert <'a> (&self, context: &Context, key: u64, subject: &Subject) -> Result<(),WriteError> {
        //println!("IndexFixed.insert({}, {:?})", key, subject );
        //TODO: this is dumb, figure out how to borrow here
        //      and replace with borrows for nested subjects
        let node = &self.root;

        // TODO: optimize index node creation so we're not changing relationship as an edit
        // after the fact if we don't strictly have to. That said, this gives us a great excuse
        // to work on the consistency model, so I'm doing that first.

        self.recurse_set(context, 0, key, node, subject)
    }
    // Temporarily managing our own bubble-up
    // TODO: finish moving the management of this to context / context::subject_graph
    fn recurse_set(&self, context: &Context, tier: usize, key: u64, node: &Subject, subject: &Subject) -> Result<(),WriteError>{
        // TODO: refactor this in a way that is generalizable for strings and such
        // Could just assume we're dealing with whole bytes here, but I'd rather
        // allow for SUBJECT_MAX_RELATIONS <> 256. Values like 128, 512, 1024 may not be entirely ridiculous
        let exponent : u32 = (self.depth as u32 - 1) - tier as u32;
        let x = SUBJECT_MAX_RELATIONS.pow(exponent as u32);
        let y = ((key / (x as u64)) % SUBJECT_MAX_RELATIONS as u64) as RelationSlotId;

        //println!("Tier {}, {}, {}", tier, x, y );

        if exponent == 0 {
            //println!("]]] end of the line");
            node.set_edge(context, y as RelationSlotId,&subject)
        }else{
            match node.get_edge(context,y)? {
                Some(n) => {
                    self.recurse_set(context, tier+1, key, &n, subject)
                }
                None => {
                    let mut values = HashMap::new();
                    values.insert("tier".to_string(),tier.to_string());

                    let new_node = Subject::new( context, SubjectType::IndexNode, values )?;

                    node.set_edge(context, y, &new_node)?;

                    self.recurse_set(context, tier+1, key, &new_node, subject)
                }
            }
        }

    }
    pub fn get_root_subject_handle(&self, context: &Context) -> Result<SubjectHandle,RetrieveError> {
        Ok(SubjectHandle{
            id: self.root.id,
            subject: self.root.clone(),
            context: context.clone()
        })
    } 
    pub fn get_subject_handle(&self, context: &Context, key: u64 ) -> Result<Option<SubjectHandle>,RetrieveError> {
        match self.get(context,key)? {
            Some(subject) => {
                Ok(Some(SubjectHandle{
                    id: subject.id,
                    subject: subject,
                    context: context.clone()
                }))
            },
            None => Ok(None)
        }
    }
    pub (crate) fn get ( &self, context: &Context, key: u64 ) -> Result<Option<Subject>, RetrieveError> {
        match self.get_head( context, key )? {
            Some(mrh) => Ok(Some( context.get_subject_with_head( mrh )? )),
            None      => Ok(None)
        }
    }
    pub (crate) fn get_head ( &self, context: &Context, key: u64 ) -> Result<Option<MemoRefHead>, RetrieveError> {
        //println!("IndexFixed.get({})", key );
        //TODO: this is dumb, figure out how to borrow here
        //      and replace with borrows for nested subjects
        let mut node = self.root.clone();
        let max = SUBJECT_MAX_RELATIONS as u64;

        //let mut n;
        for tier in 0..self.depth {
            let exponent = (self.depth - 1) - tier;
            let x = max.pow(exponent as u32);
            let y = ((key / (x as u64)) % max) as RelationSlotId;
            //println!("Tier {}, {}, {}", tier, x, y );

            //println!("INDEX GET {} - {:?}", node.id, node.head.read().unwrap().memo_ids() );
            if exponent == 0 {
                //println!("]]] end of the line");
                return node.get_edge_head( context, y as RelationSlotId);

            }else{
                match node.get_edge( context, y)? {
                    Some(n) => node = n,
                    None    => return Ok(None),
                }
            }

        };

        panic!("Sanity error");

    }
}

/*
    let idx_node = Subject::new_kv(&context_b, "dummy","value").unwrap();
    idx_node.set_relation( 0, rec_b1 );
    rec_b2.set_relation( 1, rec_b1 );
*/
