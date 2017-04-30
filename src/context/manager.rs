#![warn(bad_style, missing_docs,
        unused, unused_extern_crates, unused_import_braces,
        unused_qualifications, unused_results)]

use super::*;
use memorefhead::{RelationSlotId,RelationLink};

use rculock::{RcuLock,RcuGuard};
use std::sync::Mutex;

// Major TODOs for this module:
// 1. think about interleavings between concurrent deletion and addition of a subject. Will it do the right thing?
// 2. create benchmarks to measure the relative performance of various forms of RCU
// 3. optimize

// add an item -> offset added
// remove an item -> offset reclaimed
// increment and removal of a given node must be a mutex, lest we remove an item being referenced

// QUESTION:
// Should this be an ArcCell Linked list? If so, then:
// * The increment/Decrement process could relink as necessary to sort the list (add new link before removing old)
// * And the iterator could simply traverse the links while other operations are in progress
// * Additionally: this would mean that a mutex is no longer necessary, but at the cost of more copies/Arc-fiddling


#[derive(Clone)]
struct ContextItemList{
    vacancies: Vec<ItemId>,
    items: Vec<Arc<RcuLock<Option<ContextItem>>>>
}

impl ContextItemList {
    fn new() -> Self {
        ContextItemList{
            vacancies: Vec::with_capacity(30),
            items:     Vec::with_capacity(30)
        }
    }
}

/// Performs topological sorting.
pub struct ContextManager {
    itemlist: RcuLock<ContextItemList>,
    pathology: Option<Box<Fn(String)>>
}
type ItemId = usize;

/// Graph datastore for MemoRefHeads in our query context.
///
/// The functions of ContextManager are twofold:
/// 1. Store Subject MemoRefHeads to which an observer `Context` is actually contextualized. This is in turn used
/// by relationship projection logic (Context.get_subject_with_head calls ContextManager.get_head) 
/// 2. Provide a reverse topological iterator over the MemoRefHeads present, sufficient for context compression.
///
/// Crucially: `ContextManager` *must* be able to contain, and iterate cyclic subject relations. The underlying Memo structure is immutable,
/// and thus acyclic, but cyclic `Subject` relations are permissable due to what could be thought of as the "uncommitted" content of the ContextManager.
/// For this reason, we must perform context compression as a single pass over the contained MemoRefHeads, as a cyclic relation could otherwise cause and infinite loop.
/// The internal reference count increment/decrement thus contains cycle breaker functionality.
///
/// The ContextManager has been authored with the idea in mind that it generally should not exceed O(1k) MemoRefHeads. It should be periodically compressed to control expansion.

impl <'a> ContextManager {
    pub fn new() -> ContextManager {
        ContextManager {
            itemlist: RcuLock::new(ContextItemList::new()),
            pathology: None
        }
    }
    pub fn new_pathological( pathology: Box<Fn(String)> ) -> ContextManager {
        ContextManager {
            itemlist: RcuLock::new(ContextItemList::new()),
            pathology: Some(pathology)
        }
    }

    /// Returns the number of elements in the `ContextManager`.
    #[allow(dead_code)]
    pub fn subject_count(&self) -> usize {
        self.itemlist.read().items.iter().filter(|i| i.read().is_some()).count()
    }
    #[allow(dead_code)]
    pub fn subject_head_count(&self) -> usize {
        self.itemlist.read().items.iter().filter(|i| {
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
        self.itemlist.read().vacancies.len()
    }

    /// Returns true if the `ContextManager` contains no entries.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.itemlist.read().items.is_empty()
    }

    pub fn subject_ids(&self) -> Vec<SubjectId> {
        self.itemlist.read().items
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
    pub fn contains_subject(&self, subject_id: SubjectId) -> bool {
        let itemlist = self.itemlist.read();
        if let Some(_) = Self::get_item_by_subject(&mut *itemlist, subject_id){
            true
        }else{
            false
        }
    }
    pub fn contains_subject_head(&self, subject_id: SubjectId) -> bool {
        let itemlist = self.itemlist.read();
        if let Some(item) = Self::get_item_by_subject(&mut *itemlist,subject_id){
            if let Some(_) = item.head {
                true
            }else{
                false
            }
        }else{
            false
        }
    }
    pub fn get_head(&self, subject_id: SubjectId) -> Option<MemoRefHead> {
        let itemlist = self.itemlist.read();
        if let Some(item) = Self::get_item_by_subject( &mut *itemlist, subject_id ) {
            item.head.clone()
        } else {
            None
        }
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

        let itemlist = self.itemlist.write();
        let item = self.assert_item(subject_id);

        item.head = Some(head);

        for link in relation_links {
            self.set_relation(&mut *itemlist, item, link);
        }
    }
    pub fn remove_subject_head(&mut self, subject_id: SubjectId ) {

        let itemlist = &mut *self.itemlist.write();
        if let Some(item) = Self::get_item_by_subject(itemlist, subject_id) {
            let mut relations = Vec::new();

            let decrement = 0 - (item.indirect_references + 1);
            for relation in item.relations.iter() {
                if let Some(rel_item_id) = *relation {
                    relations.push(rel_item_id);
                }
            }
        
            item.relations.clear();

            if item.indirect_references == 0 {
                // If nobody points to me, we can fully bail out
                item.remove( itemlist );
                itemlist.vacancies.push(item.id);
            }else{
                // otherwise just remove the head that we intend to remove
                item.head = None;
            }

            // no head means we're not pointing to these anymore, at least not within the context manager
            for rel_item_id in relations {
                let mut visited = vec![false; itemlist.items.len()];
                visited[item.id] = true;
                self.increment(itemlist, rel_item_id, decrement, &mut visited);
            }

        }

    }

    /// Creates or returns a ContextManager item for a given subject_id
    fn assert_item(&mut self, subject_id: SubjectId) -> ContextItem {
        // First look in read mode
        for rcu_item in self.itemlist.read().items.iter(){
            if let Some(item) = *rcu_item.read(){
                if item.subject_id == subject_id {
                    return item;
                }
            }
        }

        // If not found, then get a write lock, and double check to make sure someone else hasn't added it
        let itemlist = self.itemlist.write();

        if let Some(item) = Self::get_item_by_subject(&mut *itemlist, subject_id){
            return item
        }
        // If still no, then add it ( while still under a write lock, to ensure we haven't any interlopers )

        ContextItem::add(&mut *itemlist, subject_id, None)

    }

    /// Set or unset a relation for a given slot id
    fn set_relation(&self, itemlist: &mut ContextItemList, item: &mut ContextItem, link: RelationLink) {

        // Do we have a relation for this slot_id already?
        if let Some(&Some(rel_item_id)) = item.relations.get(link.slot_id as usize) {
            // Yes
            let rel_item = itemlist.items[rel_item_id].read().expect("sanity error. relation item_id located, but not found in items");
            if Some(rel_item.subject_id) == link.subject_id {
                // If it's the same as what we want, then bail out. do not increment or decrement
                return;
            }else{
                // Otherwise, we need to decrement it to reflect our no-longer-pointing to it
                let decrement = 0 - (1 + item.indirect_references);
                let mut visited = vec![false; itemlist.items.len()];
                visited[rel_item_id] = true;
                rel_item.increment(self, &mut itemlist, decrement, &mut visited);

                // Will actually overwrite, or un-set this below
            }
        }

        // If we actually want it to be set:
        if let Some(subject_id) = link.subject_id {
            let new_rel_item = self.assert_item(subject_id);
            item.set_relation(link.slot_id,new_rel_item);
        } else {
            // Otherwise, make sure it's unset
            // sometimes this will be unnecessary, but it's essential to overwrite a Some() if it's there
            if link.slot_id as usize >= item.relations.len() {
                item.relations.resize(link.slot_id as usize + 1, None)
            }

            item.relations[link.slot_id as usize] = None;
        }
    }
    fn get_item_by_subject(itemlist: &ContextItemList, subject_id: SubjectId) -> Option<ContextItem> {
        for (i,rcu_item) in (*itemlist.items).iter().enumerate(){
            if let Some(item) = *rcu_item.read(){ // does a clone internally, so we might as well use it
                if item.subject_id == subject_id{
                    return Some(item);
                }
            }
        }
        None
    }

    pub fn compress(&mut self, slab: &Slab) {

        for item in self.subject_iter_rev() {
            let rssh = context_item.get_relation_slot_subject_head();

            if rssh.len() > 0 {
                let memoref = slab.new_memo(
                    Some(item.subject_id),
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
    /// For a given SubjectId, Retrieve a RelationSlotSubjectHead containing all referred subject heads resident in the `ContextManager`
 /*   pub fn compress_subject(&mut self, subject_id: SubjectId) -> Option<RelationSlotSubjectHead> {
        let mut slot_item_ids = Vec::new();
        {
            if let Some(ref mut item) = Self::get_item_by_subject( subject_id ) {
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
    pub fn add_test_subject(&self, subject_id: SubjectId, maybe_relation: Option<MemoRefHead>, slab: &Slab) -> MemoRefHead {
        let rssh = if let Some(rel_head) = maybe_relation {
            RelationSlotSubjectHead::single(0, rel_head.first_subject_id().expect("subject_id not found in relation head"), rel_head.clone())
        }else{
            RelationSlotSubjectHead::empty()
        };
        let head = slab.new_memo_basic_noparent(Some(subject_id), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        self.set_subject_head(subject_id, head.project_all_relation_links(&slab), head.clone());
        head
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

#[derive(Clone)]
struct ContextItem {
    id: ItemId,
    subject_id: SubjectId,
    indirect_references: isize,
    head: Option<MemoRefHead>,
    relations: Vec<Option<ItemId>>,
}

impl ContextItem {
    // asking the ContextItem to add itself is more conducive to a potential linkedlist change in the future
    fn add(itemlist: &mut ContextItemList, subject_id: SubjectId, maybe_head: Option<MemoRefHead>) -> ContextItem {
        
        if let Some(item_id) = { itemlist.vacancies.pop() } {
            let item = ContextItem {
                id: item_id,
                subject_id: subject_id,
                head: maybe_head,
                indirect_references: 0,
                relations: Vec::new(),
            };
            itemlist.items[item_id] = Arc::new(RcuLock::new(Some(item)));
            
        } else {
            let item_id = itemlist.items.len();
            
            let item = ContextItem {
                id: item_id,
                subject_id: subject_id,
                head: maybe_head,
                indirect_references: 0,
                relations: Vec::new(),
            };

            itemlist.items.push(Arc::new(RcuLock::new(Some(item))));
            item_id
        }



    }
    // asking the ContextItem to remove itself is more conducive to a potential linkedlist change in the future
    pub fn remove(self, itemlist: &mut ContextItemList) {
        *itemlist.items[self.id].write() = None;
        itemlist.vacancies.push(self.id);
    }
    pub fn add_rel_item (&self, manager: &ContextManager, itemlist: &mut ContextItemList, rel_item: &ContextItem, slot_id: RelationSlotId ){
        if slot_id as usize >= self.relations.len() {
            self.relations.resize(slot_id as usize + 1, None)
        }
        self.relations[slot_id as usize] = Some(rel_item.id);

        let increment = 1 + self.indirect_references;

        let mut added = vec![false; itemlist.items.len()];
        rel_item.increment(manager, itemlist, increment, &mut added);
    }
    fn increment(&mut self, manager: &ContextManager, itemlist: &mut ContextItemList, increment: isize, visited: &mut Vec<bool>) {
        if let Some(pathology) = manager.pathology {
            //pathology("pre_increment".to_string());
        }

        // Avoid traversing cycles
        if Some(&true) == visited.get(self.id) {
            return; // dejavu! Bail out
        }
        visited[self.id] = true;

        let relations: Vec<ItemId>;

        let rcu_item = itemlist.items[self.id].write();
        if let Some(ref mut item) = *rcu_item {
            item.indirect_references += increment;
            if item.indirect_references == 0 && item.head.is_none(){
                *rcu_item = None;
                itemlist.vacancies.push(self.id);
            }
            assert!(item.indirect_references >= 0, "sanity error. indirect_references below zero");

            relations = item.relations.iter().filter_map(|r| *r).collect();
        }

        for rel_item in self.get_relations( itemlist ) {
            rel_item.increment(manager, itemlist, increment, visited);
        }

    }
    fn get_relations(&self, itemlist: &mut ContextItemList) -> Vec<ContextItem>{
        let out = Vec::new();
        for maybe_rel_item_id in self.relations.iter() {
            if let &Some(rel_item_id) = maybe_rel_item_id {
                if let Some(ref rel_item) = *(itemlist.items[rel_item_id]) {
                    out.push( rel_item );
                }
            }
        }

        out
    }
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

pub struct ContextSubjectItem {
    pub item_id: ItemId,
    pub subject_id: SubjectId,
    pub head: MemoRefHead,
    pub from_subject_ids: Vec<SubjectId>,
    pub to_subject_ids: Vec<SubjectId>,
    pub indirect_references: usize,
}

pub struct ContextSubjectIter {
    // The compiler thinks this is unused? Seems like a bug
    visited: Vec<ItemId>,
    manager: Arc<ContextManager>
}
impl ContextSubjectIter{
    pub fn new( manager: ContextManager ) -> Self{

        unimplemented!();
        // ContextSubjectIter{
        //     next_item: ContextItem,
        //     manager: manager
        // }
    },

}

use std::mem;
impl Iterator for ContextSubjectIter {
    type Item = ContextSubjectItem;

    fn next(&mut self) -> Option<ContextSubjectItem> {
        unimplemented!();
        //let items: &Vec<Option<Item>
        //let item = mem::replace(self.next_item, Self::next_item(0));
        //item
        // FOR now, taking the low road
        // Vec<(usize, MemoRefHead, Vec<SubjectId>)>
        // let mut subject_heads: Vec<SubjectHead> = self.manager.items.read().iter()
        //     .filter_map(|(offset, i)| {
        //         if let &Some(ref item) = i {
        //             if let Some(ref head) = item.head {

        //                 let relation_subject_ids: Vec<SubjectId> = item.relations
        //                     .iter()
        //                     .filter_map(|maybe_item_id| {
        //                         if let &Some(item_id) = maybe_item_id {
        //                             if let Some(ref item) = manager.items[item_id] {
        //                                 Some(item.subject_id)
        //                             } else {
        //                                 panic!("sanity error, subject_head_iter")
        //                             }
        //                         } else {
        //                             None
        //                         }
        //                     })
        //                     .collect();

        //                 return Some(SubjectHead {
        //                     item_id: offset,
        //                     subject_id: item.subject_id,
        //                     indirect_references: item.indirect_references as usize,
        //                     head: head.clone(),
        //                     from_subject_ids: vec![],
        //                     to_subject_ids: relation_subject_ids,
        //                 });
        //             }
        //         }
        //         None
        //     })
        //     .collect();

        // // Intentionally doing the inverse sort here because the iterator is using pop
        // // TODO: be sure to reverse this later if we switch to incremental calculation
        // if fwd {
        //     subject_heads.sort_by(|a, b| a.indirect_references.cmp(&b.indirect_references));
        // }else{
        //     subject_heads.sort_by(|a, b| b.indirect_references.cmp(&a.indirect_references));
        // }

    }
}

/// Reverse topological iterator over subject heads which are resident in the context manager

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

    #[test]
    fn context_manager_contention() {

        use std::thread;
        use std::sync::{Arc,Mutex};

        let net = Network::create_new_system();
        let slab = Slab::new(&net);

        let interloper = Arc::new(Mutex::new(1));

        let mut manager = ContextManager::new_pathological(Box::new(|caller|{
            if caller == "pre_increment".to_string() {
                interloper.lock().unwrap();
            }
        }));


        let head1 = manager.add_test_subject(1, None,        &slab);    // Subject 1 is pointing to nooobody

        let lock = interloper.lock().unwrap();
        let t1 = thread::spawn(|| {
            // should block at the first pre_increment
            let head2 = manager.add_test_subject(2, Some(head1), &slab);    // Subject 2 slot 0 is pointing to Subject 1
            let head3 = manager.add_test_subject(3, Some(head2), &slab);    // Subject 3 slot 0 is pointing to Subject 2
        });

        manager.remove_subject_head(1);
        drop(lock);

        t1.join();

        assert_eq!(manager.contains_subject(1),      true  );
        assert_eq!(manager.contains_subject_head(1), false );
        assert_eq!(manager.contains_subject_head(2), true  );
        assert_eq!(manager.contains_subject_head(3), true  );


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        
        let mut iter = manager.subject_head_iter_rev();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 1 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    
}
