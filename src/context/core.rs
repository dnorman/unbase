use super::*;
use memorefhead::{RelationSlotId,RelationLink};
use std::sync::{Arc,Mutex,RwLock};

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
    inner:      Mutex<ContextManagerInner>,
    //pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}
struct Inner{
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
}