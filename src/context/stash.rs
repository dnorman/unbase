//#![allow(dead_code)]

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
    vacancies:         Vec<ItemId>
}
type ItemId = usize;

impl Stash {
    pub fn new () -> Stash {
        Default::default()
    }
    /// Returns the number of subjects in the `Stash` including placeholders.
    pub fn _count(&self) -> usize {
        self.inner.lock().unwrap().index.len()
    }
    /// Returns the number of subject heads in the `Stash`
    pub fn _head_count(&self) -> usize {
        self.iter().count()
    }
    pub fn _vacancy_count(&self) -> usize {
        self.inner.lock().unwrap().vacancies.len()
    }
    /// Returns true if the `Stash` contains no entries.
    pub fn _is_empty(&self) -> bool {
        self.inner.lock().unwrap().items.is_empty()
    }
    pub fn subject_ids(&self) -> Vec<SubjectId> {
        self.inner.lock().unwrap().index.iter().map(|i| i.0 ).collect()
    }
    /// Returns an iterator for all MemoRefHeads presently in the stash
    pub (crate) fn iter (&self) -> StashIterator {
        StashIterator::new(&self.inner)
    }
    /// Get MemoRefHead (if resident) for the provided subject_id
    pub fn get_head(&self, subject_id: SubjectId) -> Option<MemoRefHead> {
        let inner = self.inner.lock().unwrap();

        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(StashItem{ head: Some(ref h), .. })) => {
                        Some(h.clone())
                    },
                    _ => None
                }
            }
            None => None
        }
    }
    /// Apply the a MemoRefHead to the stash, such that we may use it for consistency enforcement of later queries.
    /// Return the post-application MemoRefHead which is a product of the MRH for the subject in question which was
    /// already the stash (if any) and that which was provided. Automatically project relations for the subject in question
    /// and remove any MemoRefHeads which are referred to.
    ///
    /// Assuming tree-structured data (as is the case for index nodes) the post-compaction contents of the stash are
    /// logically equivalent to the pre-compaction contents, despite being physically smaller.
    /// Note: Only MemoRefHeads of SubjectType::IndexNode may be applied. All others will panic.
    pub fn apply_head (&self, slab: &Slab, apply_head: &MemoRefHead) -> MemoRefHead {
        // IMPORTANT! no locks may be held for longer than a single statement in this scope.
        // happens-before determination may require remote memo retrieval, which is a blocking operation.

        match apply_head.subject_type() {
            Some(SubjectType::IndexNode) => {},
            _ => {
                panic!("Only SubjectType::IndexNode may be applied to a context")
            }
        }

        // Lets be optimistic about concurrency. Calculate a new head from the existing one (if any)
        // And keep a count of edits so we can detect if we collide with anybody.
        // We need to play this game because the stash is used by everybody, and we can't hold a lock on its internals for long.
        // It is conceivable that this may be substantially improved once the stash internals are switched to use atomics.
        // Of course that's a whole other can of worms.
        loop {
            let subject_id = apply_head.subject_id().unwrap();

            // Get the head and editcount for this specific subject id.
            let (new_head, edit_counter) = match self.get_head_and_editcount(subject_id) {
                Some((mut head,ec)) => {

                    // Will likely block here due to MemoRef traversal as needed for happens-before determination
                    let did_apply = head.apply(apply_head, slab);

                    if !did_apply {
                        return head.clone(); // Nothing applied, no need to go on – We're done here
                    }
                    (head,ec)
                },
                None => {
                    // Not present means 0 edits.
                    // Note that the edit counter starts at 1 on item creation
                    ( apply_head.clone(),0 )
                }
            };

            let links = new_head.project_all_edge_links_including_empties(slab); // May block here

            if { self.try_set_head( subject_id, new_head.clone(), &links, edit_counter ) } {

                for link in links.iter() {
                    if let EdgeLink::Occupied{ ref head, .. } = *link {
                        self.prune_head(slab, head);
                    }
                }

            }else{
                continue;
            }

            return new_head;
        }


    }
    // Prune a subject head from the `Stash` if it's descended by compare_head
    pub fn prune_head (&self, slab: &Slab, compare_head: &MemoRefHead) -> bool {

        if let &MemoRefHead::Subject{ subject_id, .. } = compare_head {
            loop{
                if let Some((ref head,edit_counter)) = self.get_head_and_editcount(subject_id){
                    if compare_head.descends(head, slab) { // May block here
                        if self.try_remove_head( subject_id, edit_counter ) {
                            return true;
                        }else{
                            continue;
                        }
                    }
                }

                break;
            }
        }

        false
    }
    /// Try to set a subject head, aborting if additional edits have occurred since the provided `StashItem` edit counter
    fn try_set_head (&self, subject_id: SubjectId, new_head: MemoRefHead, links: &Vec<EdgeLink>, edit_counter: usize) -> bool {
        let mut inner   = self.inner.lock().unwrap();

        let item_id = inner.assert_item(subject_id);
        {
            let item = inner.items[item_id].as_mut().unwrap();
            if item.edit_counter != edit_counter {
                return false;
            }
        }

        for link in links.iter() {
            match link {
                &EdgeLink::Vacant{slot_id} => {
                    let mut item = inner.items[item_id].as_mut().unwrap();
                    if let Some(rel_item_id) = item.relations[slot_id as usize]{
                        // TODO: Not sure what exactly ought to occur here
                    }
                    item.relations[ slot_id as usize ] = None;
                },
                &EdgeLink::Occupied{slot_id, head: ref rel_head} => {
                    if let &MemoRefHead::Subject{ subject_id: rel_subject_id, .. } = rel_head {
                        let rel_item_id = inner.assert_item(rel_subject_id);

                        let mut item = inner.items[item_id].as_mut().unwrap();
                        item.relations[ slot_id as usize ] = Some(rel_item_id);
                    }
                }
            }
        }

        {
            let mut item = inner.items[item_id].as_mut().unwrap();
            item.head = Some(new_head);
            item.edit_counter += 1;
        }

        true
    }
    /// Try to remove a subject head, aborting if additional edits have occurred since the provided `StashItem` edit counter
    fn try_remove_head (&self, subject_id: SubjectId, edit_counter: usize ) -> bool {
        let mut inner = self.inner.lock().unwrap();

        if let Some(item_id) = inner.get_item_id_for_subject(subject_id){
            {
                let mut item = inner.items[item_id].as_mut().unwrap();
                if item.edit_counter != edit_counter {
                    return false;
                }

                item.edit_counter += 1; // probably pointless
            }
            inner.remove_item(item_id);
        }

        true
    }
    fn get_head_and_editcount(&self, subject_id: SubjectId) -> Option<(MemoRefHead, usize)> {
        let inner = self.inner.lock().unwrap();
        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(StashItem{ head: Some(ref h), ref edit_counter, .. })) => {
                        Some((h.clone(), *edit_counter))
                    },
                    _ => None
                }
            }
            None => None
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
    fn assert_item(&mut self, subject_id: SubjectId) -> ItemId {

        let index = &mut self.index;
        match index.binary_search_by(|x| x.0.cmp(&subject_id) ){
            Ok(i) => {
                index[i].1
            }
            Err(i) =>{
                let item = StashItem::new(subject_id, None);

                let item_id = if let Some(item_id) = self.vacancies.pop() {
                    self.items[item_id] = Some(item);
                    item_id
                } else {
                    self.items.push(Some(item));
                    self.items.len() - 1
                };

                index.insert(i, (subject_id, item_id));
                item_id
            }
        }
    }
    fn remove_item(&mut self, item_id: ItemId) {
        self.items[item_id] = None;
        self.vacancies.push(item_id);

        if let Ok(i) = self.index.binary_search_by(|x| x.1.cmp(&item_id) ){
            self.index.remove(i);
        }
    }
}

// struct StashItemSlot{
//     item_id: ItemId,
//     edit_count: usize,
// }

struct StashItem {
    subject_id:   SubjectId,
    head:         Option<MemoRefHead>,
    relations:    Vec<Option<ItemId>>,
    edit_counter: usize,
}

impl StashItem {
    fn new(subject_id: SubjectId, maybe_head: Option<MemoRefHead>) -> Self {
        StashItem {
            subject_id: subject_id,
            head: maybe_head,
            relations: Vec::new(),
            edit_counter: 1 // Important for existence to count as an edit, as it cannot be the same as non-existence (0)
        }
    }
    // pub fn vacate_relation_slot(&self, inner: &StashInner, slot_id: RelationSlotId ){
    //     if let Some(rel_item_id) = self.relations[slot_id as usize] {
    //         let relation = inner.items[rel_item_id];
    //         // remove the item unless it was edited
    //         if relation.edit_count = last_edit_count {
    //             inner.cull_item(rel_item_id)
    //         }
    //     }
    // }
}

pub (crate) struct StashIterator {
    inner: Arc<Mutex<StashInner>>,
    visited: Vec<SubjectId>,
}

impl StashIterator {
    fn new (inner: &Arc<Mutex<StashInner>>) -> Self {
        StashIterator{
            inner: inner.clone(),
            visited: Vec::with_capacity(inner.lock().unwrap().items.len())
        }
    }
}

/// Rudimentary MemoRefHead iterator. It operates under the assumptions that:
/// 1. The contents of the stash may change mid-iteration
/// 2. We do not wish to visit two MemoRefHeads bearing the same subject_id twice
/// It's possible however that there are some circumstances where we may want to violate #2,
/// for instance if the MemoRefHead for subject X is advanced, but we are mid iteration and
/// have already issued an item for subject X. This may be a pertinent modality for some use cases.
impl Iterator for StashIterator{
    type Item = MemoRefHead;
    fn next (&mut self) -> Option<Self::Item> {
        let inner = self.inner.lock().unwrap();
        
        for item in inner.items.iter(){
            if let &Some(ref item) = item {
                if item.head.is_some() && !self.visited.contains(&item.subject_id) {
                    self.visited.push(item.subject_id);
                    return Some(item.head.clone().unwrap())
                }
            }
        }

        None
    }

}