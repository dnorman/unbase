use super::*;
use context::*;
use subject::*;
use slab::*;
use memorefhead::MemoRefHead;

use error::RetrieveError;
use std::collections::HashMap;


pub struct IndexFixed {
    pub root: SubjectCore,
    pub depth: u8
}

impl IndexFixed {
    pub fn new (context: &Context, depth: u8) -> IndexFixed {
        Self {
            root: SubjectCore::new( context, SubjectType::IndexNode, HashMap::new() ),
            depth: depth
        }
    }
    pub fn new_from_memorefhead (context: &Context, depth: u8, memorefhead: MemoRefHead ) -> IndexFixed {
        Self {
            root: SubjectCore::reconstitute( context, memorefhead ),
            depth: depth
        }
    }
    pub fn insert <'a> (&self, context: &Context, key: u64, subject: &SubjectCore) {
        //println!("IndexFixed.insert({}, {:?})", key, subject );
        //TODO: this is dumb, figure out how to borrow here
        //      and replace with borrows for nested subjects
        let node = &self.root;

        // TODO: optimize index node creation so we're not changing relationship as an edit
        // after the fact if we don't strictly have to. That said, this gives us a great excuse
        // to work on the consistency model, so I'm doing that first.

        self.recurse_set(context, 0, key, node, subject);
    }
    // Temporarily managing our own bubble-up
    // TODO: finish moving the management of this to context / context::subject_graph
    fn recurse_set(&self, context: &Context, tier: usize, key: u64, node: &SubjectCore, subject: &SubjectCore) {
        // TODO: refactor this in a way that is generalizable for strings and such
        // Could just assume we're dealing with whole bytes here, but I'd rather
        // allow for SUBJECT_MAX_RELATIONS <> 256. Values like 128, 512, 1024 may not be entirely ridiculous
        let exponent : u32 = (self.depth as u32 - 1) - tier as u32;
        let x = SUBJECT_MAX_RELATIONS.pow(exponent as u32);
        let y = ((key / (x as u64)) % SUBJECT_MAX_RELATIONS as u64) as RelationSlotId;

        //println!("Tier {}, {}, {}", tier, x, y );

        if exponent == 0 {
            // BUG: move this clause up
            //println!("]]] end of the line");
            node.set_relation(context, y as RelationSlotId,&subject);
        }else{
            match node.get_edge(context,y) {
                Ok(n) => {
                    self.recurse_set(context, tier+1, key, &n, subject);

                    //TEMPORARY - to be replaced by automatic context compaction
                    node.set_relation(context, y, &n);
                }
                Err( RetrieveError::NotFound ) => {
                    let mut values = HashMap::new();
                    values.insert("tier".to_string(),tier.to_string());

                    let new_node = SubjectCore::new( context, SubjectType::IndexNode, values );
                    node.set_relation(context, y,&new_node);

                    self.recurse_set(context, tier+1, key, &new_node, subject);

                    //TEMPORARY - to be replaced by automatic context compaction
                    node.set_relation(context, y, &new_node);
                }
                _ => {
                    panic!("unhandled error")
                }
            }
        }

    }
    pub fn get ( &self, context: &Context, key: u64 ) -> Result<SubjectCore, RetrieveError> {

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

            if exponent == 0 {
                //println!("]]] end of the line");
                return node.get_edge( context, y as RelationSlotId);

            }else{
                if let Ok(n) = node.get_edge( context, y){
                    node = n;
                }else{
                    return Err(RetrieveError::NotFound);
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
