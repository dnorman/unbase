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

        // TODO2: Add an in-progress count to guard against the remove-and-readd scenario.
        //        Could possibly implement this as a phantom refcount increment of the head in question.
        //
        //   Example: We get head and edit count, then do stuff for a while.
        //            Somebody comes along and deletes the subject, and somebody re-adds it.
        //            We finish doing stuff, and compare the edit count, which matches, even though it shouldn't.
        //
        // ORRRR: Just followthrough with the lockfree descendant-invariant design & do away with the
        //        silly edit count comparison (Better, but harder)

        match apply_head.subject_type() {
            Some(SubjectType::IndexNode) => {},
            _ => {
                panic!("Only SubjectType::IndexNode may be applied to a context")
            }
        }

        // Lets be optimistic about concurrency. Calculate a new head from the existing one (if any)
        // And keep a count of edits so we can detect if we collide with anybody. We need to play this
        // game because the stash is used by everybody. We need to sort out happens-before for MRH.apply_head,
        // and to project relations. Can't hold a lock on its internals for nearly long enough to do that, lest
        // we run into deadlocks, or just make other threads wait. It is conceivable that this may be
        // substantially improved once the stash internals are switched to use atomics. Of course that's a
        // whole other can of worms.

        let subject_id = apply_head.subject_id().unwrap();

        loop {
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
                    // NOTE: The edit counter is initialized at 1 on item creation to differentiate non-presence (0) from presence (1+)
                    ( apply_head.clone(),0 )
                }
            };

            // It is inappropriate here to do a contextualized projection (one which considers the current context stash)
            // and the head vs stash descends check would always return true, which is not useful for pruning.
            let links = new_head.noncontextualized_project_all_edge_links_including_empties(slab); // May block here due to projection memoref traversal

            if self.try_set_head( subject_id, new_head.clone(), &links, edit_counter ) {
                // It worked!

                for link in links.iter() {
                    if let EdgeLink::Occupied{ ref head, .. } = *link {
                        // Prune subject heads which are descended by their parent nodes (topographical parent, not causal)
                        // Once the stash is atomic/lock-free we should do this inside of set StashInner.set_relation.
                        // For now has to be separated out into a different step because happens-before may require
                        // memo-retrieval (blocking) and the stash innards currently require locking.

                        self.prune_head(slab, head);

                        // we aren't projecting the edge links using the context. Why?
                    
                    }
                }

            }else{
                // Somebody beat us to the punch. Go around and give it another shot
                // consider putting a random thread sleep here?
                continue;
            }

            return new_head;
        }


    }
    // TODO2: Change to get_head (..) -> Option<ItemGuard>
    fn get_head_and_editcount(&self, subject_id: SubjectId) -> Option<(MemoRefHead, usize)> {
        let inner = self.inner.lock().unwrap();
        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(StashItem{ head: Some(ref h), ref edit_counter, .. })) => {
                        //TODO2 Really need to figure out how to make ItemGuard work, but that would require Stash inner to be an Rc.
                        Some((h.clone(), *edit_counter)) 
                        // Some(ItemGuard::new(item_id, h.clone(), inner.clone())
                    },
                    _ => None
                }
            }
            None => None
        }
    }
    /// Try to set a subject head, aborting if additional edits have occurred since the provided `StashItem` edit counter
    fn try_set_head (&self, subject_id: SubjectId, new_head: MemoRefHead, edge_links: &Vec<EdgeLink>, edit_counter: usize) -> bool {
        let mut inner = self.inner.lock().unwrap();

        let item_id = inner.assert_item(subject_id);
        {
            // make sure the edit counter hasn't been incremented since the get_head_and_editcount
            let item = inner.items[item_id].as_ref().unwrap();
            if item.edit_counter != edit_counter {
                return false;
            }
        }
        // Ok! nobody got in our way – Lets do this...

        // record all the projected relations for the new head
        for edge_link in edge_links.iter() {
            match edge_link {
                &EdgeLink::Vacant{slot_id} => {
                    inner.set_relation(item_id, slot_id, None);
                },
                &EdgeLink::Occupied{slot_id, head: ref rel_head} => {
                    if let &MemoRefHead::Subject{ subject_id: rel_subject_id, .. } = rel_head {
                        let rel_item_id = inner.assert_item(rel_subject_id);
                        inner.set_relation(item_id, slot_id, Some(rel_item_id));
                    }
                }
            }
        }

        // set the new head itself
        {
            let item = inner.items[item_id].as_mut().unwrap();
            item.head = Some(new_head);
            item.edit_counter += 1;
        }

        true
    }

    /// Prune a subject head from the `Stash` if it's descended by compare_head
    ///
    /// The point of this function is to feed it the (non-contextual projected) child-edge from a parent tree node.
    /// If it descends what we have in the stash then the contents of the stash are redundant, and can be removed.
    /// The logical contents of the stash are the same before and after the removal of the direct contents, thus allowing
    //  compaction without loss of meaning.
    pub fn prune_head (&self, slab: &Slab, compare_head: &MemoRefHead) -> bool {

        // compare_head is the non contextualized-projection of the edge head
        if let &MemoRefHead::Subject{ subject_id, .. } = compare_head {
            loop{
                if let Some((ref head,edit_counter)) = self.get_head_and_editcount(subject_id){
                    if compare_head.descends(head, slab) { // May block here
                        if self.try_remove_head( subject_id, edit_counter ) {
                            // No interlopers. We were successful
                            return true;
                        }else{
                            // Ruh roh, some sneaky sneak made a change since we got the head
                            // consider putting a random thread sleep here?
                            continue;
                        }
                    }
                }

                break;
            }
        }

        false
    }
    /// Try to remove a subject head, aborting if additional edits have occurred since the provided `StashItem` edit counter
    fn try_remove_head (&self, subject_id: SubjectId, edit_counter: usize ) -> bool {
        let mut inner = self.inner.lock().unwrap();

        let mut remove : Option<ItemId> = None;
        {
            if let Some(item_id) = inner.get_item_id_for_subject(subject_id){
                let item = inner.items[item_id].as_mut().unwrap();
                if item.edit_counter != edit_counter {
                    return false;
                }

                item.edit_counter += 1;
                remove = Some(item_id);
            }
        }
        if let Some(item_id) = remove {
            inner.unset_head(item_id);
        }

        true
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
    fn set_relation (&mut self, item_id: ItemId, slot_id: RelationSlotId, maybe_rel_item_id: Option<ItemId>){
        let mut decrement : Option<ItemId> = None;
        {
            let item = self.items[item_id].as_mut().unwrap();

            // we have an existing relation in this slot
            if let Some(ex_rel_item_id) = item.relations[slot_id as usize]{
                // If its the same as we're setting it to, then bail out
                if Some(ex_rel_item_id) == maybe_rel_item_id {
                    return;
                }

                // otherwise we need to decrement the previous occupant
                decrement = Some(ex_rel_item_id);
            };
        }

        if let Some(decrement_item_id) = decrement {
            self.decrement_item(decrement_item_id);
        }

        // Increment the new (and different) relation
        if let Some(rel_item_id) = maybe_rel_item_id {
            self.increment_item(rel_item_id);
        }

        // Set the actual relation item id
        let item = self.items[item_id].as_mut().unwrap();
        item.relations[ slot_id as usize ] = maybe_rel_item_id;


    }
    fn increment_item(&mut self, item_id: ItemId){
        let item = self.items[item_id].as_mut().expect("increment_item on None");
        item.ref_count += 1;
    }
    fn decrement_item(&mut self, item_id: ItemId) {
        {
            let item = self.items[item_id].as_mut().expect("deccrement_item on None");
            item.ref_count -= 1;
        }
        self.conditional_remove_item(item_id);
    }
    fn unset_head(&mut self, item_id: ItemId){
        {
            let item = self.items[item_id].as_mut().expect("deccrement_item on None");
            item.head = None;
        }
        self.conditional_remove_item(item_id);
    }
    fn conditional_remove_item(&mut self, item_id: ItemId) {
        let remove = {
            let item = self.items[item_id].as_ref().expect("increment_item on None");
            item.ref_count == 0 && item.head.is_none()
        };

        if remove {
            self.items[item_id] = None;
            self.vacancies.push(item_id);

            if let Ok(i) = self.index.binary_search_by(|x| x.1.cmp(&item_id) ){
                self.index.remove(i);
            }
        }
    }
}

struct StashItem {
    subject_id:   SubjectId,
    head:         Option<MemoRefHead>,
    relations:    Vec<Option<ItemId>>,
    edit_counter: usize,
    ref_count:    usize,
}

impl StashItem {
    fn new(subject_id: SubjectId, maybe_head: Option<MemoRefHead>) -> Self {
        StashItem {
            subject_id: subject_id,
            head: maybe_head,
            relations: Vec::new(),
            edit_counter: 1, // Important for existence to count as an edit, as it cannot be the same as non-existence (0)
            ref_count: 0
        }
    }
}
// Tentative design for ItemGuard
// struct ItemGuard {
//     item_id: ItemId,
//     head: MemoRefHead,
//     edit_count: usize,
//     inner: Rc<StashInner>
// }
// impl ItemGuard{
//     fn new (item_id: ItemId, inner: &mut StashInner, stash: Stash) -> Self {
//         inner.increment_item(item_id);
//         ItemGuard{
//             item_id, stash
//         }
//     }
// }
// impl Drop for ItemGuard{
//     fn drop(&mut self) {
//         let inner = self.stash.inner.lock().unwrap();
//         inner.decrement_item(self.item_id);
//     }
// }

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