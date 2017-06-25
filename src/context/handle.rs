// mod manager;
// mod subject_graph;
// mod topo_subject_head_iter;

use slab::*;
use subject::*;
use memorefhead::MemoRefHead;
use error::RetrieveError;
use index::IndexFixed;

use std::ops::Deref;
use std::fmt;
use std::collections::HashMap;
use std::sync::{Mutex, RwLock, Arc, Weak};

#[derive(Clone)]
pub struct ContextHandle(pub Arc<ContextCore>);

/// User-exposed handle for a query context.
/// Only functionality to be exposed to the user should be defined here
impl ContextHandle {
    /// Retrieves a subject by ID from this context only if it is currently resedent
    fn get_subject_if_resident(&self, subject_id: SubjectId) -> Option<Subject> {

        if let Some(weaksub) = self.subjects.read().unwrap().get(&subject_id) {
            if let Some(subject) = weaksub.upgrade() {
                // NOTE: In theory we shouldn't need to apply the current context
                //      to this subject, as it shouldddd have already happened
                return Some(subject);
            }
        }

        None
    }
    /// Retrive a Subject from the root index by ID
    pub fn get_subject_by_id(&self, subject_id: SubjectId) -> Result<Subject, RetrieveError> {

        match *self.root_index.read().unwrap() {
            Some(ref index) => index.get(subject_id),
            None => Err(RetrieveError::IndexNotInitialized),
        }
    }

    

    // Magically transport subject heads into another context in the same process.
    // This is a temporary hack for testing purposes until such time as proper context exchange is enabled
    // QUESTION: should context exchanges be happening constantly, but often ignored? or requested? Probably the former,
    //           sent based on an interval and/or compaction ( which would also likely be based on an interval and/or present context size)
    pub fn hack_send_context(&self, other: &Self) -> usize {
        self.compress();

        let from_slabref = self.slab.my_ref.clone_for_slab(&other.slab);

        let mut memoref_count = 0;

        // for subject_head in self.manager.subject_head_iter() {
        //     memoref_count += subject_head.head.len();
        //     other.apply_subject_head(subject_head.subject_id,
        //                              &subject_head.head
        //                                  .clone_for_slab(&from_slabref, &other.slab, false),
        //                              true);
        //     // HACK inside a hack - manually updating the remote subject is cheating, but necessary for now because subjects
        //     //      have a separate MRH versus the context
        // }

        memoref_count
    }
    pub fn get_subject_head(&self, subject_id: SubjectId) -> Option<MemoRefHead> {
        unimplemented!();
        // if let Some(ref head) = self.manager.get_head(subject_id) {
        //     Some((*head).clone())
        // } else {
        //     None
        // }
    }
    pub fn get_subject_head_memo_ids(&self, subject_id: SubjectId) -> Vec<MemoId> {
        if let Some(head) = self.get_subject_head(subject_id) {
            head.memo_ids()
        } else {
            vec![]
        }
    }
    pub fn cmp(&self, other: &Self) -> bool {
        // stable way:
        &*(self.0) as *const _ != &*(other.0) as *const _

        // unstable way:
        // Arc::ptr_eq(&self.inner,&other.inner)
    }
    pub fn weak(&self) -> WeakContext {
        WeakContext(Arc::downgrade(&self.0))
    }

    /// Attempt to compress the present query context.
    /// We do this by issuing Relation memos for any subject heads which reference other subject heads presently in the query context.
    /// Then we can remove the now-referenced subject heads, and repeat the process in a topological fashion, confident that these
    /// referenced subject heads will necessarily be included in subsequent projection as a result.
    pub fn compress(&self){
        unimplemented!()
        //self.manager.compress(&self.slab);
    }

    pub fn is_fully_materialized(&self) -> bool {
        unimplemented!();
        // for subject_head in self.manager.subject_head_iter() {
        //     if !subject_head.head.is_fully_materialized(&self.slab) {
        //         return false;
        //     }
        // }
        // return true;

    }
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        // println!("# ContextShared.drop");
    }
}
impl fmt::Debug for Context {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        unimplemented!();

        // fmt.debug_struct("ContextShared")
        //     .field("subject_heads", &self.manager.subject_ids() )
        //     // TODO: restore Debug for WeakSubject
        //     //.field("subjects", &self.subjects)
        //     .finish()
    }
}

impl WeakContext {
    pub fn upgrade(&self) -> Option<Context> {
        match self.0.upgrade() {
            Some(i) => Some(Context(i)),
            None => None,
        }
    }
    pub fn cmp(&self, other: &WeakContext) -> bool {
        if let Some(context) = self.upgrade() {
            if let Some(other) = other.upgrade() {
                // stable way:
                &*(context.0) as *const _ != &*(other.0) as *const _

                // unstable way:
                // Arc::ptr_eq(&context.inner,&other.inner)
            } else {
                false
            }
        } else {
            false
        }


    }
}
