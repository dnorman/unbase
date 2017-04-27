mod manager;
// mod subject_graph;
// mod topo_subject_head_iter;

use slab::*;
use subject::*;
use memorefhead::MemoRefHead;
use error::RetrieveError;
use index::IndexFixed;
use self::manager::ContextManager;

use std::ops::Deref;
use std::fmt;
use std::collections::HashMap;
use std::sync::{Mutex, RwLock, Arc, Weak};

#[derive(Clone)]
pub struct Context(pub Arc<ContextInner>);

impl Deref for Context {
    type Target = ContextInner;
    fn deref(&self) -> &ContextInner {
        &*self.0
    }
}

pub struct ContextInner {
    pub slab: Slab,
    pub root_index: RwLock<Option<IndexFixed>>,

    /// For compaction of the subject_heads
    manager: ContextManager,

    /// For active subjects / subject subscription management
    subjects: RwLock<HashMap<SubjectId, WeakSubject>>,
}

#[derive(Clone)]
pub struct WeakContext(Weak<ContextInner>);

#[derive(Clone)]
pub enum ContextRef {
    Weak(WeakContext),
    Strong(Context),
}

impl ContextRef {
    pub fn get_context<'a>(&'a self) -> Context {
        match self {
            &ContextRef::Weak(ref c) => {
                c.upgrade().expect("Sanity error. Weak context has been dropped")
            }
            &ContextRef::Strong(ref c) => c.clone(),
        }
    }
}

impl Context {
    pub fn new(slab: &Slab) -> Context {

        let new_self = Context(Arc::new(ContextInner {
            slab: slab.clone(),
            root_index: RwLock::new(None),
            manager: ContextManager::new(),
            subjects: RwLock::new(HashMap::new()),
        }));

        // Typically subjects, and the indexes that use them, have a hard link to their originating
        // contexts. This is useful because we want to make sure the context (and associated slab)
        // stick around until we're done with them

        // The root index is a bit of a special case however, because the context needs to have a hard link to it,
        // as it must use the index directly. Therefore I need to make sure it doesn't have a hard link back to me.
        // This shouldn't be a problem, because the index is private, and not subject to direct use, so the context
        // should outlive it.

        let seed = slab.get_root_index_seed().expect("Uninitialized slab");

        let index = IndexFixed::new_from_memorefhead(ContextRef::Weak(new_self.weak()), 5, seed);

        *new_self.root_index.write().unwrap() = Some(index);

        new_self
    }
    pub fn insert_into_root_index(&self, subject_id: SubjectId, subject: &Subject) {
        if let Some(ref index) = *self.root_index.write().unwrap() {
            index.insert(subject_id, subject);
        } else {
            panic!("no root index")
        }
    }

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

    /// Retrieve a subject for a known MemoRefHead – ususally used for relationship traversal.
    /// Any relevant context will also be applied when reconstituting the relevant subject to ensure that our consistency model invariants are met
    pub fn get_subject_with_head(&self,
                                 subject_id: SubjectId,
                                 mut head: MemoRefHead)
                                 -> Result<Subject, RetrieveError> {
        // println!("# Context.get_subject_with_head({},{:?})", subject_id, head.memo_ids() );

        if head.len() == 0 {
            return Err(RetrieveError::InvalidMemoRefHead);
        }

        if let Some(relevant_context_head) = self.manager.get_head(subject_id) {
            // println!("# \\ Relevant context head is ({:?})", relevant_context_head.memo_ids() );
            head.apply(&relevant_context_head, &self.slab);

        } else {
            // println!("# \\ No relevant head found in context");
        }

        match self.get_subject_if_resident(subject_id) {
            Some(ref mut subject) => {
                subject.apply_head(&head);
                return Ok(subject.clone());
            }
            None => {}
        }

        // NOTE: Subject::reconstitute calls back to Context.subscribe_subject()
        //       so we need to release the mutex prior to this
        let subject = Subject::reconstitute(ContextRef::Strong(self.clone()), head);
        return Ok(subject);

    }
    /// Subscribes a resident subject struct to relevant updates from this context
    /// Used by the subject constructor
    pub fn subscribe_subject(&self, subject: &Subject) {
        // println!("Context.subscribe_subject({})", subject.id );
        {
            self.subjects.write().unwrap().insert(subject.id, subject.weak());
        }
        self.slab.subscribe_subject(subject.id, self);
    }
    /// Unsubscribes the subject from further updates. Used by Subject.drop
    /// ( Temporarily defeated due to deadlocks. TODO )
    pub fn unsubscribe_subject(&self, subject_id: SubjectId) {
        // println!("# Context.unsubscribe_subject({})", subject_id);
        // let _ = subject_id;
        self.subjects.write().unwrap().remove(&subject_id);

        // BUG/TODO: Temporarily disabled unsubscription
        // 1. Because it was causing deadlocks on the context AND slab mutexes
        // when the thread in the test case happened to drop the subject
        // when we were busy doing apply_subject_head, which locks context,
        // and is called by slab – so clearly this is untenable
        // 2. It was always sort of a hack that the subject was managing subscriptions
        // in this way anyways. Lets put together a more final version of the subscriptions
        // before we bother with fixing unsubscription
        //
        // {
        // let mut shared = self.inner.shared.lock().unwrap();
        // shared.subjects.remove( &subject_id );
        // }
        //
        // self.inner.slab.unsubscribe_subject(subject_id, self);
        // println!("# Context.unsubscribe_subject({}) - FINISHED", subject_id);
        //

    }

    /// Called by the Slab whenever memos matching one of our subscriptions comes in, or by the Subject when an edit is made
    pub fn apply_subject_head(&self,
                              subject_id: SubjectId,
                              apply_head: &MemoRefHead,
                              notify_subject: bool) {
        // println!("Context.apply_subject_head({}, {:?}) ", subject_id, head.memo_ids() );

        {

            let head: MemoRefHead =
                if let Some(mut head) = self.manager.conditional_apply_head(subject_id, apply_head, &self.slab) {
                    head
                } else {
                    apply_head.clone()
                };

            if notify_subject {
                if let Some(ref subject) = self.get_subject_if_resident(subject_id) {
                    subject.apply_head(&head);
                }
            }

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

        for subject_head in self.manager.subject_head_iter_rev() {
            memoref_count += subject_head.head.len();

            other.apply_subject_head(subject_head.subject_id,
                                     &subject_head.head
                                         .clone_for_slab(&from_slabref, &other.slab, false),
                                     true);
            // HACK inside a hack - manually updating the remote subject is cheating, but necessary for now because subjects
            //      have a separate MRH versus the context
        }

        memoref_count
    }
    pub fn get_subject_head(&self, subject_id: SubjectId) -> Option<MemoRefHead> {
        if let Some(head) = self.manager.get_head(subject_id) {
            Some(head)
        } else {
            None
        }
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

        // starting context: [C <- B <- A, X]
        // Materialize C if needed, then replace C
        // Materialize B to point to C, then replace B and remove C
        // Materialize A to point to B, then replace A and remove B
        // End context [A, X]

        // QUESTION: Is this safe? Might need to add things to the ContextManager in the middle of this lock

        self.manager.compress(&self.slab);

    }

    /*
    pub fn compress_and_materizlie(&self) {


        // TODO: conditionalize this on the basis of the present context size

        let parent_repoints : HashMap<SubjectId,RelationSlotSubjectHead>
        // Iterate the contextualized subject heads in reverse topological order
        for subject_head in {
            self.manager.subject_head_iter()
        } {

            // TODO: implement MemoRefHead.conditionally_materialize such that the materialization threshold is selected dynamically.
            //       It shold almost certainly not materialize with a single edit since the last FullyMaterialized memo
            // head.conditionally_materialize( &self.slab );

            if subject_head.from_subject_ids.len() > 0 {
                // OK, somebody is pointing to us, so lets issue an edit for them
                // to point to the new materialized memo for their relevant relations


                for (from_head) in subject_head.referring_heads.iter(){

                    let memoref = self.slab.new_memo(
                        Some(self.id),
                        head.clone(),
                        MemoBody::Relation(RelationSlotSubjectHead(memoref_map))
                    );

                    head.apply_memoref(&memoref, &slab);
                }

         /*       self.repoint_subject_relations(subject_head.subject_id,
                                               subject_head.head,
                                               subject_head.from_subject_ids);

        */
                // NOTE: In order to remove a subject head from the context, we must ensure that
                //       ALL referencing subject heads in the context get repointed. It's not enough to just do one

                // Now that we know they are pointing to the new materialized MemoRefHead,
                // and that the resident subject struct we have is already updated, we can
                // remove this subject MemoRefHead from the context head, because subsequent
                // index/graph traversals should find this updated parent.
                //
                // When trying to materialize/compress fully (not that we'll want to do this often),
                // this would continue all the way to the root index node, and we should be left
                // with a very small context head

            }
        }

    }
    */

    pub fn is_fully_materialized(&self) -> bool {

        for subject_head in self.manager.subject_head_iter_rev() {
            if !subject_head.head.is_fully_materialized(&self.slab) {
                return false;
            }
        }

        return true;

    }
}

impl Drop for ContextInner {
    fn drop(&mut self) {
        // println!("# ContextShared.drop");
    }
}
impl fmt::Debug for Context {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {

        fmt.debug_struct("ContextShared")
            .field("subject_heads", &self.manager.subject_ids() )
            // TODO: restore Debug for WeakSubject
            //.field("subjects", &self.subjects)
            .finish()
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
