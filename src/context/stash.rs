
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
type ItemId = usize;


impl Stash {
    pub fn new () -> Stash {
        Default::default()
    }
    
    /// Returns the number of subjects in the `Stash` including placeholders.
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().index.len()
    }
    /// Returns the number of subject heads in the `Stash`
    pub fn head_count(&self) -> usize {
        self.inner.lock().unwrap().items.iter().filter(|i| {
            if let &&Some(ref item) = i {
                if let Some(_) = item.head{
                    return true;
                }
            }
            false
        }).count()
    }
    pub fn vacancy_count(&self) -> usize {
        self.inner.lock().unwrap().vacancies.len()
    }
    /// Returns true if the `Stash` contains no entries.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().items.is_empty()
    }
    pub fn subject_ids(&self) -> Vec<SubjectId> {
        self.inner.lock().unwrap().index.iter().map(|i| i.0 ).collect()
    }
    /// Get MemoRefHead (if resident) for the provided subject_id
    pub fn get_head(&mut self, subject_id: SubjectId) -> Option<MemoRefHead> {
        let inner = self.inner.lock().unwrap();

        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(StashItem{ head: Some(ref h), .. })) => {
                        Some(h.clone())
                    },
                    None => None
                }
            }
            None => None
        }
    }
    /// Apply the provided MemoRefHead for a given subject. Project relation references and add placeholders as needed
    pub fn apply_head (&self, slab: &Slab, subject_id: SubjectId, apply_head: &MemoRefHead) -> MemoRefHead {
        // IMPORTANT! no locks may be held in this scope.
        // happens-before determination may require remote memo retrieval, which is a blocking operation.

        match apply_head.subject_type(slab) {
            Some(SubjectType::IndexNode) => {

                loop {
                    let (new_head, edit_counter) = match self.get_head_and_editcount(subject_id) {
                        Some((head,ec)) => {
                            if !head.apply(apply_head, slab) { // May block here
                                return head.clone();           // Nothing applied, bail out
                            }
                            (head,ec)
                        },            
                        None => ( apply_head.clone(),0 )
                    };

                    let links = new_head.project_all_edge_links_including_empties(slab); // May block here

                    if self.try_set_head( subject_id, new_head, &links, edit_counter ) {

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
        let inner   = self.inner.lock().unwrap();
        let item_id = inner.assert_item(subject_id);
        let mut item = inner.items[item_id].as_mut().unwrap();

        if item.edit_counter != edit_counter {
            return false;
        }

        for link in links.iter() {
            match *link {
                EdgeLink::Vacant{ slot_id} => {
                    if let Some(rel_item_id) = item.relations[slot_id as usize]{
                        // Not sure what exactly ought to occur here
                    }
                    item.relations[ slot_id as usize ] = None;
                },
                EdgeLink::Occupied{slot_id, head: rel_head} => {
                    if let MemoRefHead::Subject{ subject_id: rel_subject_id, .. } = rel_head {
                        let rel_item_id = inner.assert_item(rel_subject_id);
                        item.relations[ slot_id as usize ] = Some(rel_item_id);
                    }
                }
            }
        }

        item.head = Some(new_head);
        item.edit_counter += 1;

        true
    }

    /// Try to remove a subject head, aborting if additional edits have occurred since the provided `StashItem` edit counter
    fn try_remove_head (&self, subject_id: SubjectId, edit_counter: usize ) -> bool {
        let inner   = self.inner.lock().unwrap();

        if let Some(item_id) = inner.get_item_id_for_subject(subject_id){
            let item = inner.items[item_id].as_ref().unwrap();
            if item.edit_counter != edit_counter {
                return false;
            }

            item.edit_counter += 1; // probably pointless
            inner.remove_item(item_id);
        }

        true
    }
    fn get_head_and_editcount(&mut self, subject_id: SubjectId) -> Option<(MemoRefHead, usize)> {
        let inner = self.inner.lock().unwrap();
        match inner.get_item_id_for_subject(subject_id) {
            Some(item_id) => {
                match inner.items.get(item_id) {
                    Some(&Some(item @ StashItem{ head: Some(h), .. })) => {
                        Some((h.clone(), item.edit_counter))
                    },
                    None => None
                }
            }
            None => None
        }
    }
}

/// Head Application Strategy:
/// * Assume only tree nodes may be inserted into the Stash, IE single parent (SubjectType::IndexNode)
/// * Lock, Get the current head and edit counter for the subject in question, unlock.
/// * Optimisically apply the head to the existing head (Blocking, may require remote traversal)
/// * Project all relation heads for the subject head (Blocking, may require remote traversal)
/// * ???
/// * Profit!

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
            edit_counter: 0
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