
use super::*;

/// Stash of Subject MemoRefHeads which must be considered for state projection
#[derive(Default)]
pub (in super) struct Stash{
    inner: Arc<Mutex<StashInner>>
}
#[derive(Default)]
pub (in super) struct StashInner{
    items:             Vec<Option<StashItem>>,
    index:             Vec<(SubjectId,ItemId)>,
    vacancies:         Vec<ItemId>,
    pending_vacancies: Vec<ItemId>,
    iter_counter:      usize,
    edit_counter:      usize
}
struct StashItem {
    subject_id:   SubjectId,
    refcount:     usize,
    head:         Option<MemoRefHead>,
    relations:    Vec<Option<ItemId>>,
    edit_counter: usize,
}
type ItemId = usize;


impl Stash {
    pub fn new () -> Stash {
        Default::default()
    }
    
    /// Returns the number of subjects in the `Stash` including placeholders.
    pub fn count(&self) -> usize {
        self.index.len()
    }
    /// Returns the number of subject heads in the `Stash`
    pub fn head_count(&self) -> usize {
        self.items.iter().filter(|i| {
            if let &&Some(ref item) = i {
                if let Some(_) = item.head{
                    return true;
                }
            }
            false
        }).count()
    }
    pub fn vacancy_count(&self) -> usize {
        self.vacancies.len()
    }
    /// Returns true if the `Stash` contains no entries.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
    pub fn subject_ids(&self) -> Vec<SubjectId> {
        self.index.iter().map(|i| i.0 ).collect()
    }

    /// Apply the provided MemoRefHead for a given subject. Project relation references and add placeholders as needed to the context
    pub fn apply_head (&self, slab: &Slab, subject_id: SubjectId, apply_head: &MemoRefHead, notify_subject: bool) -> MemoRefHead {
        // IMPORTANT! no locks may be held in this scope.
        // happens-before determination may require remote memo retrieval, which is a blocking operation.

        let subject_type = apply_head.subject_type(slab);
        if let SubjectType::IndexNode = subject_type {
            loop {
                let (maybe_head, edit_counter) = self.get_head_and_editcount(subject_id);
                let new_head = match maybe_head {
                    Some(head) => { head.apply(apply_head, slab); head },
                    None       => { apply_head.clone()                 }
                };
                let all_head_links_including_empties = new_head.project_all_head_links_including_empties(slab);

                if self.conditional_set_head( subject_id, new_head, all_head_links_including_empties, edit_counter ) {
                    // prune removable heads from stash
                    break;
                }
                
            }
        }

        // LEFT OFF HERERERERERERERERERERER
        // Think more aboot this
        // if let SubjectType::IndexNode = {
        //     // lock
        //     //     get existing head and edit number / generation
        //     // unlock
        //     // project child relations
        //     // iterate over any resident children
        //     // expunge them if they descend or equal the references
        // }else{
        //     // iterate over resident parent relation(s?)
        //     // NoOp if they descend or are equal
        // }

        if notify_subject {
            // TODO: Convert this to use channels / proper subscriptions
            // if let Some(ref subject) = self.get_subject_if_resident(subject_id) {
            //     subject.apply_head(apply_head)
            // }
        }
    }
    fn conditional_set_head (&self, subject_id: SubjectId, new_head: MemoRefHead, head_links: Vec<RelationLink>, edit_counter: usize) -> bool {
        // Ok, no projection or happens-before determination after this
        let inner   = self.inner.lock().unwrap();
        let item_id = inner.assert_item(subject_id);

        if let Some(ref item) = inner.items[item_id] {
            if item.edit_counter != edit_counter {
                // Something has changed. Time for a do-over.
                return false;
            }

            // TODO: Iterate only the relations that changed between the old head and the apply head

            // For now, we are assuming that all slots are present:
            for link in head_links {
                if let Some(rel_item_id) = item.relations[link.slot_id as usize] {
                    // Existing relation
                    let mut rel_item = inner.items[rel_item_id].as_ref().expect("inner.items sanity error");

                    if Some(subject_id) != link.subject_id {
                        // OK, so we're unlinking from this
                        inner.decrement_item(rel_item);
                        item.relations[link.slot_id as usize] = None;
                    }

                    // 
                    if let Some(ref new_rel_head) = link.head {
                        // >>>> PROBLEM 1 <<<< Needs to be outside of a lock
                        // >>>> PROBLEM 2 <<<< Needs be execute for new relationships, not just existing
                        if let Some(ref existing_rel_head) = rel_item.head {
                            if new_rel_head.descends(existing_rel_head, slab) {

                            }
                        }
                    }
                }else{
                    // PROBLEM! needs to be run 
                    // NON-existing relation
                    let item_id = inner.assert_item(link.subject_id);
                    let mut rel_item = inner.items[rel_item_id];
                    inner.increment_item(&*rel_item);
                    rel_item.relations[link.slot_id] = Some(item_id);
                }
            }

            return head;
        }else{
            // shouldn't ever get here
            panic!("sanity error - missing item");
        }
    }
    fn get_head_and_editcount(&mut self, subject_id: SubjectId) -> (Option<MemoRefHead>, usize) {
        let inner = self.inner.lock().unwrap();
        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(item @ StashItem{ head: Some(h), .. })) => {
                        (Some( h.clone() ), item.edit_counter)
                    },
                    None => (None,0)
                }
            }
            None => (None,0)
        }
    }
}

impl StashInner {
    /// Fetch item id for a subject if present
    pub fn get_item_id_for_subject(&self, subject_id: SubjectId ) -> Option<ItemId>{
        match self.index.binary_search_by(|x| x.0.cmp(&subject_id)){
            Ok(i)  => Some(self.index[i].1),
            Err(_) => None
        }
    }
}