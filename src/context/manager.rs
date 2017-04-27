#![warn(bad_style, missing_docs,
        unused, unused_extern_crates, unused_import_braces,
        unused_qualifications, unused_results)]

use super::*;
use memorefhead::{RelationSlotId,RelationLink};

use rculock::RcuLock;
use std::sync::Mutex;
// TODO: farm the guts of this out to it's own topo-sort accumulator crate
//      using gratuitous Arc<Mutex<>> for now which will later be converted to unsafe Mutex<Rc<Item>>

type ItemId = usize;

#[derive(Clone)]
struct ContextItem {
    subject_id: SubjectId,
    indirect_references: isize,
    head: Option<MemoRefHead>,
    relations: Vec<Option<ItemId>>,
}


/// Performs topological sorting.
pub struct ContextManager {
    items: RcuLock<Vec<Arc<RcuLock<Option<ContextItem>>>>>,
    vacancies: Mutex<Vec<ItemId>>,
}

impl ContextItem {
    fn new(subject_id: SubjectId, maybe_head: Option<MemoRefHead>) -> Self {
        ContextItem {
            subject_id: subject_id,
            head: maybe_head,
            indirect_references: 0,
            relations: Vec::new(),
        }
    }
}

/// Graph datastore for MemoRefHeads in our query context.
///
/// The functions of ContextManager are twofold:
/// 1. Store Subject MemoRefHeads to which an observe `Context` is actually contextualized. This is in turn used
/// by relationship projection logic (Context.get_subject_with_head calls ContextManager.get_head) 
/// 2. Provide a reverse topological iterator over the MemoRefHeads present, sufficient for context compression.
///
/// Crucially: `ContextManager` *must* be able to contain, and iterate cyclic subject relations. The underlying Memo structure is immutable,
/// and thus acyclic, but cyclic `Subject` relations are permissable due to what could be thought of as the "uncommitted" content of the ContextManager.
/// For this reason, we must perform context compression as a single pass over the contained MemoRefHeads, as a cyclic relation could otherwise cause and infinite loop.
/// The internal reference count increment/decrement thus contains cycle breaker functionality.
///
/// The ContextManager has been authored with the idea in mind that it generally should not exceed O(1k) MemoRefHeads. It should be periodically compressed to control expansion.
impl ContextManager {
    pub fn new() -> ContextManager {
        ContextManager {
            items: RcuLock::new(Vec::with_capacity(30)),
            vacancies: Mutex::new(Vec::with_capacity(30)),
        }
    }

    /// Returns the number of elements in the `ContextManager`.
    #[allow(dead_code)]
    pub fn subject_count(&self) -> usize {
        self.items.read().iter().filter(|i| i.read().is_some()).count()
    }
    #[allow(dead_code)]
    pub fn subject_head_count(&self) -> usize {
        self.items.read().iter().filter(|i| {
            if let Some(ref item) = *i.read() {
                if let Some(_) = item.head{
                    return true;
                }
            }
            false
        }).count()
    }
    #[allow(dead_code)]
    pub fn vacancies(&self) -> usize {
        self.vacancies.lock().unwrap().len()
    }

    /// Returns true if the `ContextManager` contains no entries.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.items.read().is_empty()
    }

    pub fn subject_ids(&self) -> Vec<SubjectId> {
        self.items.read()
            .iter()
            .filter_map(|i| {
                if let Some(ref item) = *i.read() {
                    Some(item.subject_id)
                } else {
                    None
                }
            })
            .collect()
    }
    pub fn get_head(&self, subject_id: SubjectId) -> Option<MemoRefHead> {
        // Had to table this due to double-borrow
        //pub fn get_head<'a>(&'a mut self, subject_id: SubjectId) -> Option<&'a mut MemoRefHead> {
        if let Some(item) = self.get_item_by_subject( subject_id ) {
            item.head.clone()
        } else {
            None
        }

        /* if let Some(&mut Some(ref mut item)) =
            self.items.read().iter().find(|i| {
                if let &&mut Some(ref it) = i {
                    it.subject_id == subject_id
                } else {
                    false
                }
            }) {
                item.head.clone()
            }else{
                None
            }*/
    }
    /// For a given SubjectId, apply a MemoRefHead to the one stored in the ContextManager, but only if a MemoRefHead was already present.
    /// Return Some(applied head) or None if none was present for the provided SubjectId
    pub fn conditional_apply_head (&self, subject_id: SubjectId, apply_head: &MemoRefHead, slab: &Slab) -> Option<MemoRefHead> {
        // NOTE: Ensure that any locks are not held while one head is being applied to the previous
        //       because happens-before determination may require memo traversal, which is a blocking operation.

        unimplemented!();
        /*    let relation_links = head.project_all_relation_links(&self.slab);
            {
                self.manager.set_subject_head(subject_id, relation_links, head.clone());
            }
        */
        //.apply(apply_head, &self.slab);
        //head.clone()
    }

    /// Update the head for a given subject. The previous head is summarily overwritten.
    /// Any mrh.apply to the previous head must be done externally, if desired
    /// relation_links must similarly be pre-calculated
    pub fn set_subject_head(&mut self,
                            subject_id: SubjectId,
                            relation_links: Vec<RelationLink>,
                            head: MemoRefHead) {
        let item_id = {
            self.assert_item(subject_id)
        };
        if let Some(ref mut item) = self.items[item_id] {
            item.head = Some(head);
        }

        for link in relation_links {
            self.set_relation(item_id, link);
        }
    }
    pub fn remove_subject_head(&mut self, subject_id: SubjectId ) {
        if let Some(item_id) = self.items.iter().position(|i| {
            if let &Some(ref it) = i {
                it.subject_id == subject_id
            } else {
                false
            }
        }) {
            let mut full_remove = false;
            let mut relations = Vec::new();
            let decrement;
            let items_len = self.items.len();

            {
                if let Some(ref mut item) = self.items[item_id] {
                    decrement = 0 - (item.indirect_references + 1);
                    for relation in item.relations.iter() {
                        if let Some(rel_item_id) = *relation {
                            relations.push(rel_item_id);
                        }
                    }
                
                    item.relations.clear();

                    if item.indirect_references == 0 {
                        // If nobody points to me, we can fully bail out
                        full_remove = true;
                    }else{
                        // otherwise just remove the head that we intend to remove
                        item.head = None;
                    }
                }else{
                    panic!("sanity error");
                }

                if full_remove {
                    self.items[item_id] = None;
                    self.vacancies.push(item_id);
                }
            }

            // no head means we're not pointing to these anymore, at least not within the context manager
            for rel_item_id in relations {
                let mut removed = vec![false; items_len];
                self.increment(rel_item_id, decrement, &mut removed);
            }

        }

    }

    /// Creates or returns a ContextManager item for a given subject_id
    fn assert_item(&mut self, subject_id: SubjectId) -> ItemId {
        if let Some(item_id) = self.items.iter().position(|i| {
            if let &Some(ref it) = i {
                it.subject_id == subject_id
            } else {
                false
            }
        }) {
            item_id
        } else {
            let item = ContextItem::new(subject_id, None);

            if let Some(item_id) = self.vacancies.pop() {
                self.items[item_id] = Some(item);
                item_id
            } else {
                self.items.push(Some(item));
                self.items.len() - 1
            }

        }
    }

    fn set_relation(&mut self, item_id: ItemId, link: RelationLink) {

        // let item = &self.items[item_id];
        // retrieve existing relation by SlotId as the vec offset
        // Some(&Some()) due to empty vec slot vs None relation (logically equivalent)
        let mut remove = None;
        {
            let item = {
                if let Some(ref item) = self.items[item_id] {
                    item
                } else {
                    panic!("sanity error. set relation on item that does not exist")
                }
            };

            if let Some(&Some(rel_item_id)) = item.relations.get(link.slot_id as usize) {
                // relation exists

                let decrement;
                {
                    if let &Some(ref rel_item) = &self.items[rel_item_id] {

                        // no change. bail out. do not increment or decrement
                        if Some(rel_item.subject_id) == link.subject_id {
                            return;
                        }

                        decrement = 0 - (1 + item.indirect_references);
                    } else {
                        panic!("sanity error. relation item_id located, but not found in items")
                    }
                }

                remove = Some((rel_item_id, decrement));
            };
        }


        // ruh roh, we're different. Have to back out the old relation
        // (a little friendly sparring with the borrow checker :-x )
        if let Some((rel_item_id, decrement)) = remove {
            let mut removed = vec![false; self.items.len()];
            {
                self.increment(rel_item_id, decrement, &mut removed)
            };
            // item.relations[link.slot_id] MUST be set below
        }

        if let Some(subject_id) = link.subject_id {
            let new_rel_item_id = {
                self.assert_item(subject_id)
            };

            let increment;
            {
                if let &mut Some(ref mut item) = &mut self.items[item_id] {
                    while item.relations.len() <= link.slot_id as usize { 
                        item.relations.push(None);
                    }

                    item.relations[link.slot_id as usize] = Some(new_rel_item_id);
                    increment = 1 + item.indirect_references;
                } else {
                    panic!("sanity error. relation just set")
                }
            };

            let mut added = vec![false; self.items.len()];
            self.increment(new_rel_item_id, increment, &mut added);
        } else {
            // sometimes this will be unnecessary, but it's essential to overwrite a Some() if it's there
            if let &mut Some(ref mut item) = &mut self.items[item_id] {
                while item.relations.len() <= link.slot_id as usize { 
                    item.relations.push(None);
                }

                item.relations[link.slot_id as usize] = None;

            } else {
                panic!("sanity error. relation item not found in items")
            }
        }
    }
    fn increment(&mut self, item_id: ItemId, increment: isize, seen: &mut Vec<bool>) {
        // Avoid traversing cycles
        if Some(&true) == seen.get(item_id) {
            return; // dejavu! Bail out
        }
        seen[item_id] = true;

        let relations: Vec<ItemId>;
        let mut remove = false;
        {
            if let &mut Some(ref mut item) = &mut self.items[item_id] {
                item.indirect_references += increment;
                if item.indirect_references == 0 && item.head.is_none(){
                    remove = true;
                }
                assert!(item.indirect_references >= 0,
                        "sanity error. indirect_references below zero");

                relations = item.relations.iter().filter_map(|r| *r).collect();
            } else {
                panic!("sanity error. increment for item_id");
            }
        };

        if remove {
            self.items[item_id] = None;
            self.vacancies.push(item_id);
        }

        for rel_item_id in relations {
            self.increment(rel_item_id, increment, seen);
        }

    }
    fn get_item_by_subject<'a>(&'a mut self, subject_id: SubjectId) -> Option<&'a ContextItem> {
        if let Some(item) =
            (*self.items.read()).iter().find(|i| {
                if let Some(ref it) = *i.read() {
                    it.subject_id == subject_id
                } else {
                    false
                }
            }) {
                Some(item)
            }else{
                None
            }
    }
    // I don't really like have this be internal to the manager, but there's presently no item_id based interface and I'm not sure if there will be.
    // It's inefficient to have to convert back and forth between SubjectId and ItemId.
    pub fn compress(&mut self, slab: &Slab) {

        for subject_head in self.subject_head_iter_rev() {
            // TODO: think about whether we should limit ourselves to doing this only for resident subjects with heads

            let items = self.items.read();
            let item_count = items.len();
            if let Some(ref item) = items[subject_head.item_id].read() {
                let mut rssh = RelationSlotSubjectHead::empty();

                for (slot_id, maybe_rel_item_id) in item.relations.iter().enumerate(){
                    if let Some(rel_item_id) = *maybe_rel_item_id {
                        if let Some(ref mut rel_item) = items[rel_item_id] {
                            let decrement = 0 - (item.indirect_references + 1);
                            if let Some(head) = rel_item.head.take() {
                                rssh.insert(slot_id as RelationSlotId, rel_item.subject_id, head);
                                
                                let mut removed = vec![false; item_count];
                                self.increment(rel_item_id, decrement, &mut removed);
                            }
                        }
                    }
                }

                if rssh.len() > 0 {
                    let memoref = slab.new_memo(
                        Some(subject_head.subject_id),
                        subject_head.head.clone(),
                        MemoBody::Relation( rssh )
                    );

                    let new_head = memoref.to_head();

                    // WARNING: we're inside a mutex here. Any memos we request as a part of the projection process will likely not arrive due to waiting on this mutex.
                    // FIX IT! FIX IT!
                    let relation_links = new_head.project_all_relation_links(&slab);
                    self.set_subject_head( subject_head.subject_id, relation_links , new_head );

                }
            }
        }
    }
    /// For a given SubjectId, Retrieve a RelationSlotSubjectHead containing all referred subject heads resident in the `ContextManager`
 /*   pub fn compress_subject(&mut self, subject_id: SubjectId) -> Option<RelationSlotSubjectHead> {
        let mut slot_item_ids = Vec::new();
        {
            if let Some(ref mut item) = self.get_item_by_subject( subject_id ) {
                for (slot_id, maybe_rel_item_id) in item.relations.iter().enumerate(){
                    if let Some(rel_item_id) = *maybe_rel_item_id {
                        slot_item_ids.push((slot_id,rel_item_id));
                    }
                }
            }
        }
        
        if slot_item_ids.len() > 0 {

            let mut rssh = RelationSlotSubjectHead::empty();

            for (slot_id,rel_item_id) in slot_item_ids {
                if let Some(ref mut rel_item) = self.items[rel_item_id] {
                    if let Some(ref head) = rel_item.head {
                        rssh.insert(slot_id as RelationSlotId, rel_item.subject_id, head.clone());
                    }
                }
            }
            Some(rssh)
        }else {
            None
        }
    }
    */
    #[allow(dead_code)]
    pub fn subject_head_iter_fwd(&self) -> SubjectHeadIter {
        SubjectHeadIter::new(self, true)
    }
    pub fn subject_head_iter_rev(&self) -> SubjectHeadIter {
        SubjectHeadIter::new(self, false)
    }
    /*fn rel_item_head_iter<'a>(&'a self, item: &'a ContextItem) -> RelItemHeadIter<'a> {
        RelItemHeadIter{
            offset: 0,
            len: item.relations.len(),
            rel_item_ids: &item.relations,
            all_items: &self.items
        }
    }*/
}
/*
use core;

// Abandoned this due to double-borrow problem
struct RelItemHeadIter<'a>  {
    offset: usize,
    len: usize,
    rel_item_ids: &'a Vec<Option<ItemId>>,
    all_items: &'a Vec<Option<ContextItem>>
}
impl<'a> Iterator for RelItemHeadIter<'a> {
    type Item = (RelationSlotId,SubjectId,MemoRefHead);
    fn next(&mut self) -> Option<Self::Item>{
        
        while self.offset < self.len {

            if let Some(rel_item_id) = self.rel_item_ids[self.offset] {
                if let Some(rel_item) = self.all_items[rel_item_id] {
                    if let Some(head) = rel_item.head {
                        return Some((self.offset as RelationSlotId, rel_item.subject_id, head));
                    }
                }
            }

            self.offset += 1;
        }
        None
    }
}
*/

pub struct SubjectHead {
    item_id: ItemId,
    pub subject_id: SubjectId,
    pub head: MemoRefHead,
    pub from_subject_ids: Vec<SubjectId>,
    pub to_subject_ids: Vec<SubjectId>,
    pub indirect_references: usize,
}

pub struct SubjectHeadIter {
    // The compiler thinks this is unused? Seems like a bug
    #[allow(dead_code)]
    sorted: Vec<SubjectHead>,
}
impl Iterator for SubjectHeadIter {
    type Item = SubjectHead;

    fn next(&mut self) -> Option<SubjectHead> {
        self.sorted.pop()
    }
}

/// Reverse topological iterator over subject heads which are resident in the context manager
impl SubjectHeadIter {
    fn new(manager: &ContextManager, fwd: bool) -> Self {
        // TODO: make this respond to context changes while we're mid-iteration.
        // Approach A: switch Vec<Item> to Arc<Vec<Option<Item>>> and avoid slot reclamation until the iter is complete
        // Approach B: keep Vec<item> sorted (DESC) by indirect_references, and reset the increment whenever the sort changes

        //let items: &Vec<Option<Item>>

        // FOR now, taking the low road
        // Vec<(usize, MemoRefHead, Vec<SubjectId>)>
        let mut subject_heads: Vec<SubjectHead> = manager.items.iter().enumerate()
            .filter_map(|(offset, i)| {
                if let &Some(ref item) = i {
                    if let Some(ref head) = item.head {

                        let relation_subject_ids: Vec<SubjectId> = item.relations
                            .iter()
                            .filter_map(|maybe_item_id| {
                                if let &Some(item_id) = maybe_item_id {
                                    if let Some(ref item) = manager.items[item_id] {
                                        Some(item.subject_id)
                                    } else {
                                        panic!("sanity error, subject_head_iter")
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect();

                        return Some(SubjectHead {
                            item_id: offset,
                            subject_id: item.subject_id,
                            indirect_references: item.indirect_references as usize,
                            head: head.clone(),
                            from_subject_ids: vec![],
                            to_subject_ids: relation_subject_ids,
                        });
                    }
                }
                None
            })
            .collect();

        // Intentionally doing the inverse sort here because the iterator is using pop
        // TODO: be sure to reverse this later if we switch to incremental calculation
        if fwd {
            subject_heads.sort_by(|a, b| a.indirect_references.cmp(&b.indirect_references));
        }else{
            subject_heads.sort_by(|a, b| b.indirect_references.cmp(&a.indirect_references));
        }

        SubjectHeadIter { sorted: subject_heads }
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use {Network, Slab};
    use slab::{MemoBody, RelationSlotSubjectHead};
    use super::ContextManager;

    #[test]
    fn context_manager_basic() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        let head1 = slab.new_memo_basic_noparent(Some(1),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::empty(),
                                     })
            .to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        let head2 = slab.new_memo_basic_noparent(Some(2),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::single(0, 1, head1),
                                     })
            .to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        let head3 = slab.new_memo_basic_noparent(Some(3),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::single(0, 2, head2),
                                     })
            .to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        let head4 = slab.new_memo_basic_noparent(Some(4),
                                     MemoBody::FullyMaterialized {
                                         v: HashMap::new(),
                                         r: RelationSlotSubjectHead::single(0, 3, head3),
                                     })
            .to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4);

        let mut iter = manager.subject_head_iter_rev();
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(4, iter.next().expect("iter result 4 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }

    #[test]
    fn context_manager_dual_indegree_zero() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4);


        // 2[0] -> 1
        // 4[0] -> 3
        let mut iter = manager.subject_head_iter_rev();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert_eq!(4, iter.next().expect("iter result 4 should be present").subject_id);
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    #[test]
    fn context_manager_repoint_relation() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4.clone());

        // Repoint Subject 2 slot 0 to subject 4
        let head2_b = slab.new_memo_basic(Some(2), head2, MemoBody::Relation(RelationSlotSubjectHead::single(0,4,head4) )).to_head();
        manager.set_subject_head(4, head2_b.project_all_relation_links(&slab), head2_b);


        // 2[0] -> 1
        // 4[0] -> 3
        // Then:
        // 2[0] -> 4
        
        let mut iter = manager.subject_head_iter_rev();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert_eq!(4, iter.next().expect("iter result 4 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    #[test]
    fn context_manager_remove() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to Subject 2
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 2, head2.clone()) }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        manager.remove_subject_head(2);
        
        let mut iter = manager.subject_head_iter_rev();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    #[test]
    fn context_manager_add_remove_cycle() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut manager = ContextManager::new();

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(1, head1.project_all_relation_links(&slab), head1.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        manager.remove_subject_head(1);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 1);

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        manager.set_subject_head(2, head2.project_all_relation_links(&slab), head2.clone());

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        manager.remove_subject_head(2);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        manager.set_subject_head(3, head3.project_all_relation_links(&slab), head3.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 1);
        manager.remove_subject_head(3);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        manager.set_subject_head(4, head4.project_all_relation_links(&slab), head4);

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        manager.remove_subject_head(4);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        let mut iter = manager.subject_head_iter_rev();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert!(iter.next().is_none(), "iter should have ended");
    }
}
