use super::*;
use slab::{RelationSlotId,RelationLink};
use subject::*;
use memorefhead::*;

use std::sync::{Arc,Mutex,RwLock};
use std::ops::Deref;

type ItemId = usize;

struct Item {
    subject_id:   SubjectId,
    refcount:     usize,
    head:         Option<MemoRefHead>,
    relations:    Vec<Option<ItemId>>,
    edit_counter: usize,
}


/// Performs topological sorting.
pub struct ContextCore {
    pub slab: Slab,
    pub root_index: RwLock<Option<IndexFixed>>,
    /// For active subjects / subject subscription management
    subjects: RwLock<HashMap<SubjectId, WeakSubject>>,

    heads:      Mutex<ContextSubjectHeads>,
    //pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}
struct ContextSubjectHeads{
    items:             Vec<Option<ContextItem>>,
    index:             Vec<(SubjectId,ItemId)>,
    vacancies:         Vec<ItemId>,
    pending_vacancies: Vec<ItemId>,
    iter_counter:      usize,
    edit_counter:      usize
}

impl Item {
    fn new(subject_id: SubjectId, maybe_head: Option<MemoRefHead>) -> Self {
        ContextItem {
            subject_id: subject_id,
            head: maybe_head,
            children: Vec::new(),
            edit_counter: 0
        }
    }
}



impl ContextCore {
    pub fn new (slab: &Slab) -> Self {
        let new_self = ContextCore {
            slab: slab.clone(),
            root_index: RwLock::new(None),
            subjects: RwLock::new(HashMap::new()),
        };

        // Typically subjects, and the indexes that use them, have a hard link to their originating
        // contexts. This is useful because we want to make sure the context (and associated slab)
        // stick around until we're done with them

        // The root index is a bit of a special case however, because the context needs to have a hard link to it,
        // as it must use the index directly. Therefore I need to make sure it doesn't have a hard link back to me.
        // This shouldn't be a problem, because the index is private, and not subject to direct use, so the context
        // should outlive it.

        let seed = slab.get_root_index_seed().expect("Uninitialized slab");

        let index = IndexFixed::new_from_memorefhead(new_self, 5, seed);

        *new_self.root_index.write().unwrap() = Some(index);
    }

    pub fn insert_into_root_index(&self, subject_id: SubjectId, subject: &Subject) {
        if let Some(ref index) = *self.root_index.write().unwrap() {
            index.insert(subject_id, subject);
        } else {
            panic!("no root index")
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

        let maybe_head = {
            // Don't want to hold the lock while calling head.apply, as it could request a memo from a remote slab, and we'd deadlock
            if let Some(ref head) = self.manager.get_head(subject_id) {
                Some((*head).clone())
            } else {
                None
            }
        };

        if let Some(relevant_context_head) = maybe_head {
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
        let subject = Subject::reconstitute(&self, head);
        return Ok(subject);

    }

        /// Subscribes a resident subject struct to relevant updates from this context
    /// Used by the subject constructor
    pub fn subscribe_subject(&self, subject: &SubjectCore) {
        // println!("Context.subscribe_subject({})", subject.id );
        {
            self.subjects.write().unwrap().insert(subject.id, subject.weak());
        }

        // TODO: determine if we want to use channels, or futures streams, or what.
        //       giving out Arcs to the context doesn't seem like the way to go
        //self.slab.subscribe_subject(subject.id, self);
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
    pub fn apply_subject_head(&self, subject_id: SubjectId, apply_head: &MemoRefHead, notify_subject: bool) {
        // println!("Context.apply_subject_head({}, {:?}) ", subject_id, head.memo_ids() );

        //self.manager.apply_head(subject_id, apply_head, &self.slab);

        if notify_subject {
            if let Some(ref subject) = self.get_subject_if_resident(subject_id) {
                subject.apply_head(apply_head);
            }
        }
    }

}