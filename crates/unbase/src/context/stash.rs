//#![allow(dead_code)]
use std::{
    iter,
    mem,
    sync::{
        Arc,
        Mutex,
    },
};

use crate::{
    error::WriteError,
    head::Head,
    slab::{
        EdgeLink,
        EntityId,
        EntityType,
        SlabHandle,
        SlotId,
        MAX_SLOTS,
    },
};
/// # What is a Stash?
/// Stash is very important to the operation of unbase - It is the beating heart of its consistency model.
///
/// A Stash is a special set containing [`Head`](crate::head::Head) and their Edges which must be
/// considered for state projection. It assumes that Heads which are put into it have a *heirarchical*
/// relationship with each other. Even if that heirarchy isn't fully represented in the Stash, it must exist, and of
/// course be free of cycles.
///
/// Because each unbase node (Slab) may relocate data with other peers, it may not have sufficient data locally to
/// project state consistently when a given value is requested later. By itself, this would be way worse than eventual
/// consistency, because one minute it would be consistent, and the next it would lose edits you just made on the same
/// node. Worse still, it may never reconverge!
///
/// Most systems use a total order, or sharding (or both) to achieve consistency – but the express goal of unbase is to
/// NOT do that. Instead, we use the Stash during the projection process to make sure we have a monotonic view of the
/// world. We may not have all The data, but at least we'll know when we're missing something, and that's useful.
///
/// So, whenever we write data into the system, it must be copied to the stash, otherwise we might forget! Similarly,
/// whenever we read data from the system, we must consult the stash to make sure we're considering all the data we
/// should. Easy peasy, right?
///
/// # Stash Compaction
/// Well, there's a bit of a downside to writing all edits into the stash: It'll fill up and get huge! If we kept all
/// this data, it would be at least the same size as a full log of all edits ever made. That's no good! We want the
/// stash to be as compact as possible. But how do we do that if we're always putting stuff into it? This is actually a
/// very common design pattern. It's called an immutable/persistent data structure. You build a b-tree, and when you
/// write a record, you create a new copy of the leaf pointing to that record, and the parent node pointing to that, and
/// so on up to the root node. The whole thing is beautifully immutable, *except* for the *one* mutable reference to the
/// root node. That root node is your "Stash"
///
/// Unfortunately if we try to implement unbase this way, we'll have an explosion of traffic! Remember, we have to play
/// nicely with others. How do we merge those edits? We would have to send each other N+1 write operations (where N is
/// the depth of the B-tree) for each edit. And then we'd have to emit new edits for each of those containing the merged
/// versions of the B-tree nodes, and and then we're have to repeat the process for other edits we receive from other
/// nodes, and so on. It's a runaway cascade, commonly referred to as "Write amplification", and it's very bad.
///
/// So what can we do instead? Well, much like an immutable/persistent data structure, each edit generates a new leaf
/// node of the B-tree, and that gets put into the stash. But we don't immediately generate new parent B-tree nodes. We
/// let it simmer a bit. Whereas the "Stash" for an immutable/persistent data structure is a set of exactly *one*
/// mutable reference to the root node, our Stash is a set of *N* mutable references to different non-root (plus exactly
/// one root) B-tree nodes. We have to add new nodes whenever we do an edit, but periodically we can issue new non-leaf
/// B-tree nodes which reference one (or ideally more) leaf nodes which were in the stash. We also do this for non-leaf
/// parents, and so on, all the way up to the root node, but the frequency of our doing so drops precipitously as we get
/// closer to the root node.
///
/// *Crucially* because we put the new non-leaf node in the stash, we can remove those which it points to!
/// We can do this because we know that the B-tree must be traversed to locate the record, and the nodes we removed from
/// the stash will be considered _vicariously_ because we kept the parent node pointing to them. As such a Stash
/// automatically prunes Heads which are descended-by-or-equal-to *any* of the Edges of any Head inserted
/// into the stash.
///
/// ```
/// # use unbase::{Slab,Network,slab::EntityId,head::Head,context::stash::Stash};
/// # async_std::task::block_on(async {
/// # let net = Network::create_new_system();
/// # let slab = Slab::initialize(&net);
/// # let slabhandle = slab.handle();
///
/// let stash = Stash::new();
///
/// let head1 = stash.add_test_head(&slab, EntityId::index_test(1), vec![]).await;
/// let head2 = stash.add_test_head(&slab, EntityId::index_test(2), vec![]).await;
/// let head3 = stash.add_test_head(&slab, EntityId::index_test(3), vec![head1, Head::Null, head2])
///                  .await;
///
/// assert_eq!(stash.concise_contents(), "I3>I1,_,I2");
/// # });
/// ```
///
/// See [concise_contents](Stash::concise_contents) for a simple way to visualize the contents of the stash
#[derive(Clone, Default)]
pub struct Stash {
    inner: Arc<Mutex<StashInner>>,
}
#[derive(Default)]
pub(super) struct StashInner {
    items:     Vec<Option<StashItem>>,
    index:     Vec<(EntityId, ItemId)>,
    vacancies: Vec<ItemId>,
}
type ItemId = usize;

impl Stash {
    pub fn new() -> Stash {
        Default::default()
    }

    /// Returns the number of entities in the `Stash` including placeholders.
    pub fn _count(&self) -> usize {
        self.inner.lock().unwrap().index.len()
    }

    /// Returns the number of entity heads in the `Stash`
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

    pub fn entity_ids(&self) -> Vec<EntityId> {
        self.inner.lock().unwrap().index.iter().map(|i| i.0.clone()).collect()
    }

    /// Return a human readable description of all the [`Head`](crate::head::Head)s currently in
    /// the stash, and the other [`Head`](crate::head::Head)s referenced by their edges (in
    /// slot-positional order).
    ///
    /// This is represented as: *Entity id 1* > Relations *A,B*; *EntityID 2* > Relations *C,D*
    ///
    /// For example:
    ///
    ///   I2>I1;I7>I2,_,I4
    ///
    ///   would indicate that the stash contains:
    ///   * A Head for entity I2 (EntityType::Index) with slot 0 pointing to I1.
    ///   * A Head for entity I3 (EntityType::Index) with slot 0 pointing to I2, slot 1 pointing to nothing, slot 2 pointing to I4
    ///
    ///   Note that in this scenario, the stash _does not_ contain heads for I1 or I4, but knows of their existence.
    ///   They may have been in the stash previously, then removed because they were "descended" by and edge of another
    /// Head,   Or they may never have been added in the first place. Regardless, the stash needs to be aware of
    /// these edges.   These are "Phantom" members of the stash, and are used as placeholders in anticipation of
    /// potential future traffic   on the part of those Entities.
    #[allow(dead_code)]
    pub fn concise_contents(&self) -> String {
        let inner = self.inner.lock().unwrap();

        let mut out = Vec::with_capacity(inner.items.len());
        for &(entity_id, item_id) in inner.index.iter() {
            let item = inner.items[item_id].as_ref().unwrap();

            let mut outstring = String::new();
            if let Head::Null = item.head {
                // This is a phantom member of the stash, whose purpose is only to serve as a placeholder
                continue;
            }
            outstring.push_str(&entity_id.concise_string());

            // determine the last occupied slot so we don't have
            let last_occupied_relation_slot = item.relations.iter().enumerate().filter(|&(_, x)| x.is_some()).last();

            if let Some((slot_id, _)) = last_occupied_relation_slot {
                outstring.push_str(">");
                let relation_entity_ids: String =
                    item.relations
                        .iter()
                        .take(slot_id + 1)
                        .map(|slot| {
                            match slot {
                                &Some(ritem_id) => inner.items[ritem_id].as_ref().unwrap().entity_id.concise_string(),
                                &None => "_".to_string(),
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(",");
                outstring.push_str(&relation_entity_ids[..]);
            }

            out.push(outstring);
        }

        out.join(";")
    }

    /// Returns an iterator for all Heads presently in the stash
    pub(crate) fn iter(&self) -> StashIterator {
        StashIterator::new(&self.inner)
    }

    /// Get Head (if resident) for the provided entity_id
    pub fn get_head(&self, entity_id: EntityId) -> Head {
        let inner = self.inner.lock().unwrap();

        match inner.get_item_id_for_entity(entity_id) {
            Some(item_id) => inner.items[item_id].as_ref().unwrap().head.clone(),
            None => Head::Null,
        }
    }

    /// Apply the a Head to the stash, such that we may use it for consistency enforcement of later queries.
    /// Return the post-application Head which is a product of the Head for the entity in question which was
    /// already the stash (if any) and that which was provided. Automatically project relations for the entity in
    /// question and remove any Heads which are referred to.
    ///
    /// Assuming tree-structured data (as is the case for index nodes) the post-compaction contents of the stash are
    /// logically equivalent to the pre-compaction contents, despite being physically smaller.
    /// Note: Only Heads of EntityType::IndexNode may be applied. All others will panic.
    pub async fn apply_head(&self, slab: &SlabHandle, apply_head: &Head) -> Result<Head, WriteError> {
        // IMPORTANT! no locks may be held for longer than a single statement in this scope.
        // happens-before determination may require remote memo retrieval, which is a blocking operation.

        match apply_head.entity_id() {
            Some(EntityId { stype: EntityType::IndexNode,
                            .. }) => {},
            _ => {
                panic!("Only EntityType::IndexNode may be applied to a context. Attempted to apply {:?}",
                       apply_head)
            },
        }

        // Lets be optimistic about concurrency. Calculate a new head from the existing one (if any)
        // And keep a count of edits so we can detect if we collide with anybody. We need to play this
        // game because the stash is used by everybody. We need to sort out happens-before for Head.apply_head,
        // and to project relations. Can't hold a lock on its internals for nearly long enough to do that, lest
        // we run into deadlocks, or just make other threads wait. It is conceivable that this may be
        // substantially improved once the stash internals are switched to use atomics. Of course that's a
        // whole other can of worms.

        let entity_id = apply_head.entity_id().unwrap();

        loop {
            // Get the head and editcount for this specific entity id.
            let mut item: ItemEditGuard = self.get_head_for_edit(entity_id);

            if !item.apply_head(apply_head, slab).await? {
                return Ok(item.get_head().clone());
            }

            if let Ok(Some((head, links))) = item.try_save() {
                // It worked!

                for link in links.iter() {
                    if let EdgeLink::Occupied { ref head, .. } = *link {
                        // Prune entity heads which are descended by their parent nodes (topographical parent, not
                        // causal) Once the stash is atomic/lock-free we should do this inside
                        // of set StashInner.set_relation. For now has to be separated out into
                        // a different step because happens-before may require memo-retrieval
                        // (blocking) and the stash innards currently require locking.

                        self.prune_head(slab, head).await?;

                        // we aren't projecting the edge links using the context. Why?
                    }
                }

                return Ok(head);
            } else {
                // Somebody beat us to the punch. Go around and give it another shot
                // consider putting a random thread sleep here?
                continue;
            }
        }
    }

    fn get_head_for_edit(&self, entity_id: EntityId) -> ItemEditGuard {
        ItemEditGuard::new(entity_id, self.inner.clone())
    }

    /// Prune a entity head from the `Stash` if it's descended by compare_head
    ///
    /// The point of this function is to feed it the (non-contextual projected) child-edge from a parent tree node.
    /// If it descends what we have in the stash then the contents of the stash are redundant, and can be removed.
    /// The logical contents of the stash are the same before and after the removal of the direct contents, thus
    /// allowing
    //  compaction without loss of meaning.
    pub async fn prune_head(&self, slab: &SlabHandle, compare_head: &Head) -> Result<bool, WriteError> {
        // compare_head is the non contextualized-projection of the edge head
        if let &Head::Entity { entity_id, .. } = compare_head {
            loop {
                let mut item: ItemEditGuard = self.get_head_for_edit(entity_id);

                if compare_head.descends_or_contains(item.get_head(), slab).await? {
                    // May yield here
                    item.set_head(Head::Null, slab).await?;

                    if let Ok(Some((_head, _links))) = item.try_save() {
                        // No interlopers. We were successful
                        return Ok(true);
                    } else {
                        // Ruh roh, some sneaky sneak made a change since we got the head
                        // consider putting a random thread sleep here?
                        continue;
                    }
                }

                break;
            }
        }

        Ok(false)
    }

    /// Create a new [`Head`](crate::head::Head) for testing purposes, and immediately add it to
    /// the context Returns a clone of the newly created + added [`Head`](crate::head::Head)
    pub async fn add_test_head(&self, slab: &SlabHandle, entity_id: EntityId, relations: Vec<Head>) -> Head {
        use crate::slab::{
            EdgeSet,
            MemoBody,
            RelationSet,
        };
        use std::collections::HashMap;

        let mut edgeset = EdgeSet::empty();

        for (slot_id, head) in relations.iter().enumerate() {
            if let &Head::Entity { .. } = head {
                edgeset.insert(slot_id as SlotId, head.clone())
            }
        }

        let head = slab.new_memo(Some(entity_id),
                                 Head::Null,
                                 MemoBody::FullyMaterialized { v: HashMap::new(),
                                                               r: RelationSet::empty(),
                                                               e: edgeset,
                                                               t: entity_id.stype, })
                       .to_head();

        self.apply_head(slab, &head).await.expect("apply head")
    }
}

impl StashInner {
    /// Fetch item id for a entity if present
    pub fn get_item_id_for_entity(&self, entity_id: EntityId) -> Option<ItemId> {
        match self.index.binary_search_by(|x| x.0.cmp(&entity_id)) {
            Ok(i) => Some(self.index[i].1),
            Err(_) => None,
        }
    }

    fn assert_item(&mut self, entity_id: EntityId) -> ItemId {
        let index = &mut self.index;
        match index.binary_search_by(|x| x.0.cmp(&entity_id)) {
            Ok(i) => index[i].1,
            Err(i) => {
                let item = StashItem::new(entity_id, Head::Null);

                let item_id = if let Some(item_id) = self.vacancies.pop() {
                    self.items[item_id] = Some(item);
                    item_id
                } else {
                    self.items.push(Some(item));

                    self.items.len() - 1
                };
                index.insert(i, (entity_id, item_id));
                item_id
            },
        }
    }

    fn set_relation(&mut self, item_id: ItemId, slot_id: SlotId, maybe_rel_item_id: Option<ItemId>) {
        let mut decrement: Option<ItemId> = None;
        {
            let item = self.items[item_id].as_mut().unwrap();

            // we have an existing relation in this slot
            if let Some(ex_rel_item_id) = item.relations[slot_id as usize] {
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
        item.relations[slot_id as usize] = maybe_rel_item_id;
    }

    fn increment_item(&mut self, item_id: ItemId) {
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

    fn conditional_remove_item(&mut self, item_id: ItemId) {
        let (remove, entity_id) = {
            let item = self.items[item_id].as_ref().expect("increment_item on None");
            (item.ref_count == 0 && item.head == Head::Null, item.entity_id)
        };

        if remove {
            self.items[item_id] = None;
            self.vacancies.push(item_id);

            if let Ok(i) = self.index.binary_search_by(|x| x.0.cmp(&entity_id)) {
                self.index.remove(i);
            }
        }
    }
}

#[derive(Debug)]
struct StashItem {
    entity_id:    EntityId,
    head:         Head,
    relations:    Vec<Option<ItemId>>,
    edit_counter: usize,
    ref_count:    usize,
}

impl StashItem {
    fn new(entity_id: EntityId, head: Head) -> Self {
        StashItem { entity_id,
                    head,
                    // QUESTION: should we preallocate all possible relation slots? or manage the length of relations
                    // vec?
                    relations: iter::repeat(None).take(MAX_SLOTS).collect(),
                    edit_counter: 1, /* Important for existence to count as an edit, as it cannot be the same as
                                      * non-existence (0) */
                    ref_count: 0 }
    }
}
// Tentative design for ItemGuard
struct ItemEditGuard {
    item_id:      ItemId,
    head:         Head,
    links:        Option<Vec<EdgeLink>>,
    did_edit:     bool,
    edit_counter: usize,
    inner_arc:    Arc<Mutex<StashInner>>,
}
impl ItemEditGuard {
    fn new(entity_id: EntityId, inner_arc: Arc<Mutex<StashInner>>) -> Self {
        let (item_id, head, edit_counter) = {
            let mut inner = inner_arc.lock().unwrap();
            let item_id = inner.assert_item(entity_id);

            // Increment to refer to the ItemEditGuard's usage
            inner.increment_item(item_id);

            let item = inner.items[item_id].as_ref().unwrap();
            (item_id, item.head.clone(), item.edit_counter)
        };

        ItemEditGuard { item_id,
                        head: head.clone(),
                        links: None,
                        did_edit: false,
                        edit_counter,
                        inner_arc }
    }

    fn get_head(&self) -> &Head {
        &self.head
    }

    async fn set_head(&mut self, set_head: Head, slab: &SlabHandle) -> Result<(), WriteError> {
        self.head = set_head;
        // It is inappropriate here to do a contextualized projection (one which considers the current context stash)
        // and the head vs stash descends check would always return true, which is not useful for pruning.
        self.links = Some(self.head.project_all_edge_links_including_empties(slab).await?); // May block here due to projection memoref traversal
        self.did_edit = true;
        Ok(())
    }

    async fn apply_head(&mut self, apply_head: &Head, slab: &SlabHandle) -> Result<bool, WriteError> {
        // NO LOCKS IN HERE
        if !self.head.mut_apply(apply_head, slab).await? {
            return Ok(false);
        }
        // It is inappropriate here to do a contextualized projection (one which considers the current context stash)
        // and the head vs stash descends check would always return true, which is not useful for pruning.
        self.links = Some(self.head.project_all_edge_links_including_empties(slab).await?); // May block here due to projection memoref traversal

        self.did_edit = true;
        return Ok(true);
    }

    fn try_save(mut self) -> Result<Option<(Head, Vec<EdgeLink>)>, ()> {
        if !self.did_edit {
            return Ok(None);
        }

        let mut inner = self.inner_arc.lock().unwrap();

        {
            // make sure the edit counter hasn't been incremented since the get_head_and_editcount
            let item = inner.items[self.item_id].as_ref().unwrap();
            if item.edit_counter != self.edit_counter {
                return Err(());
            }
        }
        // Ok! nobody got in our way – Lets do this...

        // record all the projected relations for the new head
        for edge_link in self.links.as_ref().unwrap().iter() {
            match edge_link {
                &EdgeLink::Vacant { slot_id } => {
                    inner.set_relation(self.item_id, slot_id, None);
                },
                &EdgeLink::Occupied { slot_id,
                                      head: ref rel_head, } => {
                    if let &Head::Entity { entity_id: rel_entity_id,
                                           .. } = rel_head
                    {
                        let rel_item_id = inner.assert_item(rel_entity_id);
                        inner.set_relation(self.item_id, slot_id, Some(rel_item_id));
                    }
                },
            }
        }

        // set the new head itself

        let item = inner.items[self.item_id].as_mut().unwrap();
        item.head = self.head.clone();
        item.edit_counter += 1;

        // IMPORTANT - because we consume self, drop will run after we return, ths calling decrement_item
        //             which is crucial for the evaluation of item removal in the case that we
        //             just set head to Head::Null (essentially the same as unsetting)
        return Ok(Some((mem::replace(&mut self.head, Head::Null), mem::replace(&mut self.links, None).unwrap())));
    }
}
impl Drop for ItemEditGuard {
    fn drop(&mut self) {
        let mut inner = self.inner_arc.lock().unwrap();
        // decrement to reflect the destruction of our reference. This may cause
        // the item to be automatically expunged if there are no heads nor relations
        inner.decrement_item(self.item_id);
    }
}

pub struct StashIterator {
    inner:   Arc<Mutex<StashInner>>,
    visited: Vec<EntityId>,
}

impl StashIterator {
    fn new(inner: &Arc<Mutex<StashInner>>) -> Self {
        StashIterator { inner:   inner.clone(),
                        visited: Vec::with_capacity(inner.lock().unwrap().items.len()), }
    }
}

/// Rudimentary iterator for the contents of the stash. It operates under the assumptions that:
/// 1. The contents of the stash may change mid-iteration
/// 2. We do not wish to visit two Heads bearing the same entity_id twice
/// It's possible however that there are some circumstances where we may want to violate #2,
/// for instance if the Head for entity X is advanced, but we are mid iteration and
/// have already issued an item for entity X. This may be a pertinent modality for some use cases.
impl Iterator for StashIterator {
    type Item = Head;

    fn next(&mut self) -> Option<Self::Item> {
        let inner = self.inner.lock().unwrap();

        for item in inner.items.iter() {
            if let &Some(ref item) = item {
                if item.head.is_some() && !self.visited.contains(&item.entity_id) {
                    self.visited.push(item.entity_id);
                    return Some(item.head.clone());
                }
            }
        }

        None
    }
}
