mod internal;
mod stash;
mod interface;


use slab::{RelationSlotId,RelationLink};
use subject::{Subject,SubjectCore,SubjectId,SubjectType};
use memorefhead::*;
use error::*;

use std::collections::HashMap;

use index::IndexFixed;
use slab::*;
use self::stash::Stash;

use std::sync::{Arc,Mutex,RwLock};
use std::ops::Deref;

#[derive(Clone)]
pub struct Context(Arc<ContextInner>);

pub struct ContextInner {
    pub slab: Slab,
    pub root_index: RwLock<Option<IndexFixed>>,
    stash: Stash,
    //pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}

impl Deref for Context {
    type Target = ContextInner;
    fn deref(&self) -> &ContextInner {
        &*self.0
    }
}

/// TODO: Explain what a context is here
impl Context{
    pub fn new (slab: &Slab) -> Self {
        let new_self = Context(
            Arc::new(
                ContextInner{
                    slab: slab.clone(),
                    root_index: RwLock::new(None),
                    stash: Mutex::new(Stash::new())
                }
            )
        );

        let seed = slab.get_root_index_seed().expect("Uninitialized slab");
        let index = IndexFixed::new_from_memorefhead(&new_self, 5, seed);

        *new_self.root_index.write().unwrap() = Some(index);
        new_self
    }
}
