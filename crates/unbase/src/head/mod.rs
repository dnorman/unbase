pub mod serde;

use crate::{
    error::{
        RetrieveError,
        WriteError,
    },
    slab::{
        EdgeLink,
        EdgeSet,
        EntityId,
        EntityType,
        Memo,
        MemoBody,
        MemoId,
        MemoRef,
        RelationSet,
        SlabHandle,
        SlabRef,
        SlotId,
        MAX_SLOTS,
    },
};

use std::{
    collections::{
        HashMap,
        VecDeque,
    },
    fmt,
    mem,
    pin::Pin,
    slice,
};

use futures::{
    future::BoxFuture,
    task::Poll,
    FutureExt,
    Stream,
    StreamExt,
};

use crate::buffer::{
    BufferHelper,
    HeadBufElement,
};
use ::serde::{
    Deserialize,
    Serialize,
};
use itertools::Itertools;
use tracing::debug;

// TODO: consider renaming to OwnedHead
#[derive(Clone, PartialEq)]
// TODO - consider changing this to a linkedlist instead of a Vec, because MOST of the time it's going to be a single
// memoref This will allow us to save allocations, and potentially have a number of traversal operations happen entirely
// on the stack? struct Link {
//    memoref: MemoRef,
//    //90+% of the time this will be None
//    next: Option<Box<Link>>
//}

/// Head is a list of MemoRefs that constitute the "head" of a given causal chain
///
/// This "head" is rather like a git HEAD, insofar as it is intended to contain only the youngest
/// descendents of a given causal chain. It provides mechanisms for applying memorefs, or applying
/// other Heads such that the mutated list may be pruned as appropriate given the above.
pub enum Head {
    Null,
    Entity {
        owning_slabref: SlabRef,
        entity_id:      EntityId,
        head:           Vec<MemoRef>,
    },
    Anonymous {
        owning_slabref: SlabRef,
        head:           Vec<MemoRef>,
    },
}

// TODO: consider renaming to ExternalHead or something like that
pub struct HeadWithProvenance {
    pub head:    Head,
    pub slabref: SlabRef,
}

/// Head takes &SlabHandle on all calls, because it is an agent of storage and referentiality, NOT an enforcer of
/// consistency
impl Head {
    pub fn new_index(slab: &SlabHandle, values: HashMap<String, String>) -> Head {
        let id = slab.generate_entity_id(EntityType::IndexNode);

        slab.new_memo(Some(id),
                      Head::Null,
                      MemoBody::FullyMaterialized { v: values,
                                                    r: RelationSet::empty(),
                                                    e: EdgeSet::empty(),
                                                    t: EntityType::IndexNode, })
            .to_head()
    }

    #[tracing::instrument]
    pub async fn mut_apply_memoref(&mut self, new: &MemoRef, slab: &SlabHandle) -> Result<bool, WriteError> {
        // Conditionally add the new memoref only if it descends any memorefs in the head
        // If so, any memorefs that it descends must be removed
        let head = match self {
            Head::Null => {
                if let Some(entity_id) = new.entity_id {
                    *self = Head::Entity { owning_slabref: new.owning_slabref,
                                           head: vec![new.clone()],
                                           entity_id };
                } else {
                    *self = Head::Anonymous { owning_slabref: new.owning_slabref,
                                              head:           vec![new.clone()], };
                }

                return Ok(true);
            },
            Head::Anonymous { ref mut head, .. } => head,
            Head::Entity { ref mut head, .. } => head,
        };

        // Conditionally add the new memoref only if it descends any memorefs in the head
        // If so, any memorefs that it descends must be removed

        // Not suuuper in love with these flag names
        let mut new_is_descended = false;
        let mut new_descends = false;

        let mut applied = false;
        let mut replaced = false;

        // I imagine it's more efficient to iterate in reverse, under the supposition that
        // new items are more likely to be at the end, and that's more likely to trigger
        // the cheapest case: (existing descends new)

        // TODO - make this more async friendly.
        'existing: for i in (0..head.len()).rev() {
            let mut remove = false;
            {
                let ref mut existing = head[i];
                if existing == new {
                    return Ok(false); // we already had this
                } else if existing.descends(&new, &slab).await? {
                    new_is_descended = true;

                    // IMPORTANT: for the purposes of the boolean return,
                    // the new memo does not get "applied" in this case

                    // If any memo in the head already descends the newcomer,
                    // then it doesn't get applied at all punt the whole thing
                    break 'existing;
                } else if new.descends(&existing, &slab).await? {
                    new_descends = true;
                    applied = true; // descends

                    if replaced {
                        remove = true;
                    } else {
                        // Lets try real hard not to remove stuff in the middle of the vec
                        // But we only get to do this trick once, because we don't want to add duplicates
                        mem::replace(existing, new.clone());
                        replaced = true;
                    }
                }
            }

            if remove {
                // because we're descending, we know the offset of the next items won't change
                head.remove(i);
            }
        }

        if !new_descends && !new_is_descended {
            // if the new memoref neither descends nor is descended
            // then it must be concurrent

            head.push(new.clone());
            applied = true; // The memoref was "applied" to the Head
        }

        Ok(applied)
    }

    #[tracing::instrument]
    pub async fn mut_apply_memorefs(&mut self, new_memorefs: &Vec<MemoRef>, slab: &SlabHandle) -> Result<bool, WriteError> {
        let mut did_apply = false;

        for new in new_memorefs.iter() {
            if self.mut_apply_memoref(new, slab).await? {
                did_apply = true;
            }
        }

        Ok(did_apply)
    }

    #[tracing::instrument]
    pub async fn mut_apply(&mut self, other: &Head, slab: &SlabHandle) -> Result<bool, WriteError> {
        match other {
            Head::Null => Ok(false),
            Head::Anonymous { ref head, .. } => {
                let mut applied = false;
                for new in head.iter() {
                    if self.mut_apply_memoref(new, slab).await? {
                        applied = true;
                    };
                }
                Ok(applied)
            },
            Head::Entity { ref head, .. } => {
                let mut applied = false;
                for new in head.iter() {
                    if self.mut_apply_memoref(new, slab).await? {
                        applied = true;
                    };
                }
                Ok(applied)
            },
        }
    }

    /// Immutably apply a second head to this one
    #[tracing::instrument]
    pub async fn apply(&self, other: &Head, slab: &SlabHandle) -> Result<(Head, bool), WriteError> {
        let mut applied = false;
        // This is just a temporary API hack so we don't forget to make this nicer later with internal _immutability_.
        // TODO reimplement this with immutabilityZ

        let mut hack_self = self.clone();
        for new in other.iter() {
            if hack_self.mut_apply_memoref(new, slab).await? {
                applied = true;
            };
        }

        Ok((hack_self, applied))
    }

    pub async fn descends_or_contains(&self, other: &Head, slab: &SlabHandle) -> Result<bool, RetrieveError> {
        // there's probably a more efficient way to do this than iterating over the cartesian product
        // we can get away with it for now though I think
        // TODO: revisit when beacons are implemented
        match *self {
            Head::Null => Ok(false),
            Head::Entity { ref head, .. } | Head::Anonymous { ref head, .. } => {
                match *other {
                    Head::Null => Ok(false),
                    Head::Entity { head: ref other_head, .. } | Head::Anonymous { head: ref other_head, .. } => {
                        if head.len() == 0 || other_head.len() == 0 {
                            println!("ONE IS ZERO");
                            return Ok(false); // searching for positive descendency, not merely non-ascendency
                        }
                        for memoref in head.iter() {
                            for other_memoref in other_head.iter() {
                                if memoref == other_memoref {
                                    //
                                } else if !memoref.descends(other_memoref, slab).await? {
                                    return Ok(false);
                                }
                            }
                        }

                        Ok(true)
                    },
                }
            },
        }
    }

    pub fn memo_ids(&self) -> Vec<MemoId> {
        match *self {
            Head::Null => Vec::new(),
            Head::Entity { ref head, .. } | Head::Anonymous { ref head, .. } => head.iter().map(|m| m.id).collect(),
        }
    }

    pub fn memo_summary(&self) -> String {
        match *self {
            Head::Null => "Null".to_string(),
            Head::Entity { ref head, .. } | Head::Anonymous { ref head, .. } => {
                head.iter()
                    .map(|mr| {
                        if let Some(memo) = mr.get_memo_if_resident() {
                            format!("{}p[{}]={}",
                                    mr.id,
                                    memo.parents.memo_ids().iter().join(","),
                                    memo.body.summary())
                        } else {
                            format!("^{}", mr.id)
                        }
                    })
                    .join(",")
            },
        }
    }

    pub fn entity_id(&self) -> Option<EntityId> {
        match *self {
            Head::Null | Head::Anonymous { .. } => None,
            Head::Entity { entity_id, .. } => Some(entity_id),
        }
    }

    pub fn owning_slabref(&self) -> Option<SlabRef> {
        match *self {
            Head::Null => None,
            Head::Anonymous { ref owning_slabref, .. } => Some(owning_slabref.clone()),
            Head::Entity { ref owning_slabref, .. } => Some(owning_slabref.clone()),
        }
    }

    pub fn is_some(&self) -> bool {
        match *self {
            Head::Null => false,
            _ => true,
        }
    }

    pub fn to_vec(&self) -> Vec<MemoRef> {
        match *self {
            Head::Null => vec![],
            Head::Anonymous { ref head, .. } => head.clone(),
            Head::Entity { ref head, .. } => head.clone(),
        }
    }

    pub fn to_vecdeque(&self) -> VecDeque<MemoRef> {
        match *self {
            Head::Null => VecDeque::new(),
            Head::Anonymous { ref head, .. } => VecDeque::from(head.clone()),
            Head::Entity { ref head, .. } => VecDeque::from(head.clone()),
        }
    }

    pub fn len(&self) -> usize {
        match *self {
            Head::Null => 0,
            Head::Anonymous { ref head, .. } => head.len(),
            Head::Entity { ref head, .. } => head.len(),
        }
    }

    pub fn iter(&self) -> slice::Iter<MemoRef> {
        // This feels pretty stupid. Probably means something is wrong with the factorization of Head
        static EMPTY: &'static [MemoRef] = &[];

        match *self {
            Head::Null => EMPTY.iter(), // HACK
            Head::Anonymous { ref head, .. } => head.iter(),
            Head::Entity { ref head, .. } => head.iter(),
        }
    }

    #[tracing::instrument]
    fn to_stream_vecdeque(&self, slab: &SlabHandle) -> VecDeque<CausalMemoStreamItem> {
        let head = match self {
            Head::Null => return VecDeque::new(),
            Head::Anonymous { ref head, .. } => head,
            Head::Entity { ref head, .. } => head,
        };

        head.iter()
            .map(|memoref| {
                // TODO - switching to an immutable internal datastructure should mitigate the need for clones here
                CausalMemoStreamItem { fut:  memoref.clone().get_memo(slab.clone()).boxed(),
                                       memo: None, }
            })
            .collect()
    }

    #[tracing::instrument]
    pub fn causal_memo_stream(&self, slab: SlabHandle) -> CausalMemoStream {
        CausalMemoStream::from_head(&self, slab)
    }

    pub async fn is_fully_materialized(&self, slab: &SlabHandle) -> Result<bool, RetrieveError> {
        // TODO: consider doing as-you-go distance counting to the nearest materialized memo for each descendent
        //       as part of the list management. That way we won't have to incur the below computational effort.

        for memoref in self.iter() {
            let memo = memoref.clone().get_memo(slab.clone()).await?;
            match memo.body {
                MemoBody::FullyMaterialized { .. } => {},
                _ => return Ok(false),
            }
        }

        Ok(true)
    }

    /// Notify whomever needs to know that a new entity has been created
    #[tracing::instrument]
    pub async fn get_value(&mut self, slab: &SlabHandle, key: &str) -> Result<Option<String>, RetrieveError> {
        // TODO: consider creating a consolidated projection routine for most/all uses
        let mut memostream = self.causal_memo_stream(slab.clone()).boxed();
        while let Some(memo) = memostream.next().await {
            // println!("# \t\\ Considering Memo {}", memo.id );
            if let Some((values, materialized)) = memo?.get_values() {
                if let Some(v) = values.get(key) {
                    return Ok(Some(v.clone()));
                } else if materialized {
                    return Ok(None); // end of the line here
                }
            }
        }

        Err(RetrieveError::MemoLineageError)
    }

    pub async fn get_relation(&mut self, slab: &SlabHandle, key: SlotId) -> Result<Option<EntityId>, RetrieveError> {
        // println!("# Entity({}).get_relation({})",self.id,key);

        let mut memostream = self.causal_memo_stream(slab.clone());
        while let Some(memo) = memostream.next().await {
            let memo = memo?;
            if let Some((relations, materialized)) = memo.get_relations() {
                debug!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}",
                       memo.id(),
                       memo.get_parent_head(),
                       relations);

                if let Some(maybe_entity_id) = relations.get(&key) {
                    return match *maybe_entity_id {
                        Some(entity_id) => Ok(Some(entity_id)),
                        None => Ok(None),
                    };
                } else if materialized {
                    debug!("\n# \t\\ Not Found (materialized)");
                    return Ok(None);
                }
            }
        }

        debug!("Not Found");
        Err(RetrieveError::MemoLineageError)
    }

    pub async fn get_edge(&mut self, slab: &SlabHandle, key: SlotId) -> Result<Option<Head>, RetrieveError> {
        let mut memostream = self.causal_memo_stream(slab.clone());

        while let Some(memo) = memostream.next().await {
            let memo = memo?;
            if let Some((edges, materialized)) = memo.get_edges() {
                debug!("# \t\\ Considering Memo {}, Head: {:?}, Relations: {:?}",
                       memo.id(),
                       memo.get_parent_head(),
                       edges);

                if let Some(head) = edges.get(&key) {
                    // TODO POSTMERGE this is likely buggy - shouldn't we be looking at all of the memorefs in the head
                    // in case of concurrencies?

                    return Ok(Some(head.clone()));
                } else if materialized {
                    debug!("\n# \t\\ Not Found (materialized)");
                    return Ok(None);
                }
            }
        }

        debug!("Not Found");
        Err(RetrieveError::MemoLineageError)
    }

    pub async fn set_value(&mut self, slab: &SlabHandle, key: &str, value: &str) -> Result<(), WriteError> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        let entity_id = self.entity_id();

        // TODO - do this in a single swap? (fairly certain that requires unsafe)
        let mut head = Head::Null;
        std::mem::swap(self, &mut head);

        let mut new_head = slab.new_memo(entity_id, head, MemoBody::Edit(vals)).to_head();

        std::mem::swap(self, &mut new_head);

        // We shouldn't need to apply the new memoref. It IS the new head
        // self.apply_memoref(&memoref, &slab).await?;

        Ok(())
    }

    pub async fn set_relation(&mut self, slab: &SlabHandle, key: SlotId, relation: &Self) -> Result<(), WriteError> {
        // println!("# Entity({}).set_relation({}, {})", &self.id, key, relation.id);
        let mut relationset = RelationSet::empty();

        let entity_id = relation.entity_id().ok_or(WriteError::BadTarget)?;

        relationset.insert(key, entity_id);

        let entity_id = self.entity_id();

        // TODO - do this in a single swap? May require unsafe
        let mut head = Head::Null;
        std::mem::swap(self, &mut head);

        let mut new_head = slab.new_memo(entity_id, head, MemoBody::Relation(relationset)).to_head();

        std::mem::swap(self, &mut new_head);

        // We shouldn't need to apply the new memoref. It IS the new head
        // self.apply_memoref(&memoref, &slab).await?;

        Ok(())
    }

    pub fn set_edge(&mut self, slab: &SlabHandle, key: SlotId, target: Head) {
        debug!("# Entity({:?}).set_edge({}, {:?})",
               &self.entity_id(),
               key,
               target.entity_id());

        let mut edgeset = EdgeSet::empty();
        edgeset.insert(key, target);

        let entity_id = self.entity_id();

        // TODO - do this in a single swap? May require unsafe
        let mut parents = Head::Null;
        std::mem::swap(self, &mut parents);

        let mut new_head = slab.new_memo(entity_id, parents, MemoBody::Edge(edgeset)).to_head();

        std::mem::swap(self, &mut new_head);

        // We shouldn't need to apply the new memoref. It IS the new head
        // self.apply_memoref(&memoref, &slab).await?;
    }

    #[tracing::instrument]
    pub async fn get_all_memo_ids(&self, slab: SlabHandle) -> Result<Vec<MemoId>, RetrieveError> {
        let mut memostream = self.causal_memo_stream(slab);

        let mut memo_ids = Vec::new();
        while let Some(memo) = memostream.next().await {
            memo_ids.push(memo?.id());
        }
        Ok(memo_ids)
    }

    //    pub fn fully_materialize( &self, slab: &Slab ) {
    //        // TODO: consider doing as-you-go distance counting to the nearest materialized memo for each descendent
    //        //       as part of the list management. That way we won't have to incur the below computational effort.
    //    }

    // Kind of a brute force way to do this
    // TODO: Consider calculating deltas during memoref application,
    //       and use that to perform a minimum cost entity_head_link edit

    // TODO: This projection method is probably wrong, as it does not consider how to handle concurrent edge-setting
    //       this problem applies to causal_memo_stream itself really, insofar as it should return sets of concurrent
    // memos to be merged rather than individual memos This in turn raises questions about how relations should be
    // merged

    /// Project all edge links based only on the causal history of this head.
    /// The name is pretty gnarly, and this is very ripe for refactoring, but at least it says what it does.
    pub async fn project_all_edge_links_including_empties(&self, slab: &SlabHandle) -> Result<Vec<EdgeLink>, RetrieveError> {
        let mut edge_links: Vec<Option<EdgeLink>> = Vec::with_capacity(MAX_SLOTS);

        // None is an indication that we've not yet visited this slot, and that it is thus eligible for setting
        for _ in 0..MAX_SLOTS as usize {
            edge_links.push(None);
        }

        let mut memostream = self.causal_memo_stream(slab.clone());
        while let Some(memo) = memostream.next().await {
            match memo?.body {
                MemoBody::FullyMaterialized { e: ref edgeset, .. } => {
                    // Iterate over all the entries in this EdgeSet
                    for (slot_id, rel_head) in &edgeset.0 {
                        // Only consider the non-visited slots
                        if let None = edge_links[*slot_id as usize] {
                            edge_links[*slot_id as usize] = Some(match *rel_head {
                                                                     Head::Null => EdgeLink::Vacant { slot_id: *slot_id },
                                                                     _ => {
                                                                         EdgeLink::Occupied { slot_id: *slot_id,
                                                                                              head:    rel_head.clone(), }
                                                                     },
                                                                 });
                        }
                    }

                    break;
                    // Fully Materialized memo means we're done here
                },
                MemoBody::Edge(ref r) => {
                    for (slot_id, rel_head) in r.iter() {
                        // Only consider the non-visited slots
                        if let None = edge_links[*slot_id as usize] {
                            edge_links[*slot_id as usize] = Some(match *rel_head {
                                                                     Head::Null => EdgeLink::Vacant { slot_id: *slot_id },
                                                                     _ => {
                                                                         EdgeLink::Occupied { slot_id: *slot_id,
                                                                                              head:    rel_head.clone(), }
                                                                     },
                                                                 })
                        }
                    }
                },
                _ => {},
            }
        }

        let out: Vec<EdgeLink> = edge_links.iter()
                                           .enumerate()
                                           .map(|(slot_id, maybe_link)| {
                                               // Fill in the non-visited links with vacants
                                               match maybe_link {
                                                   None => EdgeLink::Vacant { slot_id: slot_id as SlotId, },
                                                   Some(ref link) => link.clone(),
                                               }
                                           })
                                           .collect();

        Ok(out)
    }

    /// Contextualized projection of edges for occupied slots
    pub async fn project_occupied_edges(&self, slab: &SlabHandle) -> Result<Vec<EdgeLink>, RetrieveError> {
        let mut visited = [false; MAX_SLOTS];
        let mut edge_links: Vec<EdgeLink> = Vec::new();

        let mut memostream = self.causal_memo_stream(slab.clone());
        'memo: while let Some(memo) = memostream.next().await {
            let memo = memo?;

            let (edgeset, last) = match memo.body {
                MemoBody::FullyMaterialized { e: ref edgeset, .. } => (edgeset, true),
                MemoBody::Edge(ref edgeset) => (edgeset, false),
                _ => continue 'memo,
            };

            for (slot_id, rel_head) in edgeset.iter() {
                // TODO POSTMERGE - I believe this is wrong. Need to consider concurrencies.
                //                  Also, should probably stop whenever we hit a FullyMaterialized

                // Only consider the non-visited slots
                if !visited[*slot_id as usize] {
                    visited[*slot_id as usize] = true;

                    match *rel_head {
                        Head::Entity { .. } | Head::Anonymous { .. } => {
                            edge_links.push(EdgeLink::Occupied { slot_id: *slot_id,
                                                                 head:    rel_head.clone(), });
                        },
                        Head::Null => {},
                    };
                }
            }

            if last {
                break;
            }
        }

        Ok(edge_links)
    }

    pub fn to_buf<E, M, S, H>(&self, helper: &H) -> HeadBufElement<E, M>
        where E: Serialize + Deserialize,
              M: Serialize + Deserialize,
              S: Serialize + Deserialize,
              H: BufferHelper<EntityToken = E, MemoToken = M, SlabToken = S>
    {
        match *self {
            Head::Null => HeadBufElement::Null,
            Head::Anonymous { ref head, .. } => {
                HeadBufElement::Anonymous { head: head.iter().map(|mr| helper.from_memo_id(&mr.id)).collect(), }
            },
            Head::Entity { ref head, ref entity_id, .. } => {
                HeadBufElement::Entity { entity: helper.from_entity_id(entity_id),
                                         head:   head.iter().map(|mr| helper.from_memo_id(&mr.id)).collect(), }
            },
        }
    }
}

impl fmt::Debug for Head {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Head::Null => fmt.debug_struct("Head::Null").finish(),
            Head::Anonymous { .. } => {
                fmt.debug_struct("Head::Anonymous")
                   // .field("memo_refs",  head )
                   .field("memos", &self.memo_summary())
                   .finish()
            },
            Head::Entity { ref entity_id, .. } => {
                fmt.debug_struct("Head::Entity")
                   .field("entity_id", &entity_id)
                   //                    .field("memo_refs",  head )
                   .field("memo", &self.memo_summary())
                   .finish()
            },
        }
    }
}

struct CausalMemoStreamItem {
    //    memoref: MemoRef,
    fut:  BoxFuture<'static, Result<Memo, RetrieveError>>,
    memo: Option<Memo>,
}

pub struct CausalMemoStream {
    queue: VecDeque<CausalMemoStreamItem>,
    slab:  SlabHandle,
}

impl fmt::Debug for CausalMemoStream {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("CausalMemoIter")
           .field("remaining", &self.queue.len())
           .finish()
    }
}

/// Plausible Memo Structure:
/// /- E -> C -\
/// G ->              -> B -> A
/// head ^    \- F -> D -/
/// Desired iterator sequence: G, E, C, F, D, B, A ( Why? )
/// Consider:                  [G], [E,C], [F,D], [B], [A]
///
/// There are some problems with this having to do with the difficulty of knowing if a FullyMaterialized Memo has any
/// concurrencies
impl CausalMemoStream {
    #[tracing::instrument]
    pub fn from_head(head: &Head, slab: SlabHandle) -> Self {
        match head.owning_slabref() {
            Some(slabref) if slabref != slab.my_ref => {
                panic!("requesting slab does not match owning slab");
            },
            _ => {},
        }

        CausalMemoStream { queue: head.to_stream_vecdeque(&slab),
                           slab }
    }
}

impl Stream for CausalMemoStream {
    type Item = Result<Memo, RetrieveError>;

    #[tracing::instrument]
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut std::task::Context) -> Poll<Option<Self::Item>> {
        // iterate over head memos
        // Unnecessarly complex because we're not always dealing with MemoRefs
        // Arguably heads should be stored as Vec<MemoRef> instead of Vec<Memo>

        if self.queue.len() == 0 {
            return Poll::Ready(None);
        }

        let mut nextheads = Vec::new();

        for item in self.queue.iter_mut() {
            // QUESTION: Is it bad to pass our context? We have to poll all of these, but only want to be
            // woken up when the *first* of these futures is ready. We only get one shot at setting the
            // context/waker though, so I think we just have to deal with that.
            if let None = item.memo {
                match item.fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(memo)) => {
                        nextheads.push(memo.get_parent_head());
                        item.memo = Some(memo);
                    },
                    Poll::Ready(Err(_e)) => {
                        panic!("TODO: how should we handle memo retrieval errors in the causal iterator? {:?}",
                               _e)
                    },
                    Poll::Pending => {},
                }
            }
        }

        for nexthead in nextheads {
            let mut foo = &mut nexthead.to_stream_vecdeque(&self.slab);
            self.queue.append(&mut foo);
        }

        // TODO -make this nicer
        if let None = self.queue[0].memo {
            return Poll::Pending;
        }

        return Poll::Ready(Some(Ok(self.queue.pop_front().unwrap().memo.unwrap())));
    }
}
