pub mod stash;

use crate::{
    entity::Entity,
    error::{
        InvalidHead,
        RetrieveError,
        WriteError,
    },
    head::Head,
    index::IndexFixed,
    slab::{
        EdgeLink,
        EdgeSet,
        EntityId,
        EntityType,
        MemoBody,
        MemoId,
        RelationSet,
        SlabHandle,
        SlotId,
    },
};

use self::stash::Stash;
use timer::Delay;

use std::{
    collections::HashMap,
    fmt,
    ops::Deref,
    sync::{
        Arc,
        Mutex,
    },
    time::{
        Duration,
        Instant,
    },
};

use futures::{
    channel::mpsc,
    future::RemoteHandle,
    StreamExt,
};

use tracing::{
    span,
    Level,
};

#[derive(Clone)]
pub struct Context(Arc<ContextInner>);

pub struct ContextInner {
    pub slab:            SlabHandle,
    pub root_index_node: Arc<Mutex<Option<Head>>>,
    _applier:            RemoteHandle<()>,
    stash:               Stash,
    // pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}

impl Deref for Context {
    type Target = ContextInner;

    fn deref(&self) -> &ContextInner {
        &*self.0
    }
}

/// TODO: Explain what a context is here
impl Context {
    #[tracing::instrument]
    pub fn new(slab: SlabHandle) -> Context {
        let stash = Stash::new();

        let (tx, mut rx) = mpsc::channel(1000);
        slab.observe_index(tx);

        let applier_slab = slab.clone();
        let applier_stash = stash.clone();

        let span = span!(Level::TRACE, "Context Applier");

        let applier: RemoteHandle<()> = crate::util::task::spawn_with_handle(async move {
            while let Some(head) = rx.next().await {
                let _guard = span.enter();

                // TODO POSTMERGE - how do we handle a head-application error on the background applier?
                // Probably shouldn't retry indefinitely.
                // Some ideas:
                // a. raise an error event to the holder of the context somehow
                // b. consider this error to apply to the next query
                // c. throw away out stash and reload it from another node
                // d. employ a healing protocol of some kind - if the data is lost, it's probably going to affect more
                // than just this context

                let _merged_head = applier_stash.apply_head(&applier_slab, &head).await.unwrap();
            }
        });

        let inner = ContextInner { slab,
                                   root_index_node: Arc::new(Mutex::new(None)),
                                   stash,
                                   _applier: applier };

        Context(Arc::new(inner))
    }

    pub async fn try_fetch_kv(&self, key: &str, val: &str) -> Result<Option<Entity>, RetrieveError> {
        // TODO implement field-specific indexes
        // if I have an index for that field {
        //    use it
        //} else if I am allowed to scan this index...
        let mut index = self.root_index().await?;

        match index.scan_first_kv(self, key, val).await? {
            Some(head) => {
                let entity = self.get_entity_from_head(head).await?;

                Ok(Some(entity))
            },
            None => Ok(None),
        }
    }

    pub async fn fetch_kv(&self, key: &str, val: &str, wait: Duration) -> Result<Entity, RetrieveError> {
        let start = Instant::now();

        self.root_index().await?;

        // TODO ASYNC NOTIFY
        loop {
            let elapsed = start.elapsed();
            if elapsed > wait {
                return Err(RetrieveError::NotFoundByDeadline);
            }

            if let Some(rec) = self.try_fetch_kv(key, val).await? {
                return Ok(rec);
            }

            Delay::new(Duration::from_millis(50)).await;
        }
    }

    /// Retrive a Entity from the root index by ID
    pub async fn get_entity_by_id(&self, entity_id: EntityId) -> Result<Option<Entity>, RetrieveError> {
        let root_index = self.root_index().await?;

        match root_index.get(&self, entity_id.hack_as_u64()).await? {
            Some(s) => {
                let sh = Entity { id:      entity_id,
                                  head:    s,
                                  context: self.clone(), };

                Ok(Some(sh))
            },
            None => Ok(None),
        }
    }

    pub fn concise_contents(&self) -> String {
        self.stash.concise_contents()
    }

    // Magically transport entity heads into another context in the same process.
    // This is a temporary hack for testing purposes until such time as proper context exchange is enabled
    // QUESTION: should context exchanges be happening constantly, but often ignored? or requested? Probably the former,
    //           sent based on an interval and/or compaction ( which would also likely be based on an interval and/or
    // present context size)
    #[tracing::instrument]
    pub async fn hack_send_context(&self, other: &Context) -> Result<usize, WriteError> {
        self.compact().await?;

        let from_slabref = other.slab.agent.localize_slabref(&self.slab.my_ref);

        let mut memoref_count = 0;

        for head in self.stash.iter() {
            memoref_count += head.len();

            let apply_head = other.slab.agent.localize_head(&head, &from_slabref, false);
            other.apply_head(&apply_head).await?;
        }

        Ok(memoref_count)
    }

    pub fn try_root_index_node(&self) -> Result<Head, RetrieveError> {
        {
            if let Some(ref node) = *self.root_index_node.lock().unwrap() {
                return Ok(node.clone());
            }
        };

        let seed = self.slab.net.get_root_index_seed(&self.slab);
        if seed.is_some() {
            *self.root_index_node.lock().unwrap() = Some(seed.clone());

            Ok(seed)
        } else {
            Err(RetrieveError::IndexNotInitialized)
        }
    }

    pub async fn root_index(&self) -> Result<IndexFixed, RetrieveError> {
        let start = Instant::now();
        // TODO centralized timeout duration config
        let wait = Duration::from_secs(5);

        loop {
            if start.elapsed() > wait {
                return Err(RetrieveError::NotFoundByDeadline);
            }

            if let Ok(node) = self.try_root_index_node() {
                let index = IndexFixed::new_from_head(5, node);
                return Ok(index);
            }

            Delay::new(Duration::from_millis(50)).await;
        }
    }

    pub fn get_resident_entity_head(&self, entity_id: EntityId) -> Head {
        self.stash.get_head(entity_id).clone()
    }

    pub fn get_resident_entity_head_memo_ids(&self, entity_id: EntityId) -> Vec<MemoId> {
        self.get_resident_entity_head(entity_id).memo_ids()
    }

    pub fn cmp(&self, other: &Self) -> bool {
        // stable way:
        &*(self.0) as *const _ != &*(other.0) as *const _

        // unstable way:
        // Arc::ptr_eq(&self.inner,&other.inner)
    }

    /// Create a new [`Head`](crate::head::Head) for testing purposes, and immediately add it to
    /// the context Returns a clone of the newly created + added [`Head`](crate::head::Head)
    pub async fn add_test_head(&self, entity_id: EntityId, edges: Vec<Head>) -> Head {
        let mut edgeset = EdgeSet::empty();

        for (slot_id, head) in edges.iter().enumerate() {
            if let &Head::Entity { .. } = head {
                edgeset.insert(slot_id as SlotId, head.clone())
            }
        }

        let head = self.slab
                       .new_memo(Some(entity_id),
                                 Head::Null,
                                 MemoBody::FullyMaterialized { v: HashMap::new(),
                                                               r: RelationSet::empty(),
                                                               e: edgeset,
                                                               t: entity_id.stype, })
                       .to_head();

        self.apply_head(&head).await.expect("apply head")
    }

    /// Attempt to compress the present query context.
    /// We do this by issuing Relation memos for any entity heads which reference other entity heads presently in the
    /// query context. Then we can remove the now-referenced entity heads, and repeat the process in a topological
    /// fashion, confident that these referenced entity heads will necessarily be included in subsequent projection
    /// as a result.
    pub async fn compact(&self) -> Result<(), WriteError> {
        //        let before = self.stash.concise_contents();

        // TODO: implement topological Head iterator for stash
        //      right now we're just doing a linear pass, which will
        //      yield sub-optimal compaction

        // iterate all heads in the stash
        for parent_head in self.stash.iter() {
            // TODO POSTMERGE - ideally we'd have a better signal for when we have
            //                  reached the end of a given concurrent set of memos
            //                  versus doing a descends test
            let mut updated_edges = EdgeSet::empty();

            // Note: project_occupied_edges DOES NOT consider the contents of the stash for this head's entity. That's
            // intentional
            for edgelink in parent_head.project_occupied_edges(&self.slab).await? {
                if let EdgeLink::Occupied { slot_id,
                                            head: edge_head, } = edgelink
                {
                    if let Some(entity_id) = edge_head.entity_id() {
                        if let stash_head @ Head::Entity { .. } = self.stash.get_head(entity_id) {
                            // looking for cases where the stash is fresher than the edge

                            if stash_head.descends_or_contains(&edge_head, &self.slab).await? {
                                updated_edges.insert(slot_id, stash_head);
                            }
                        }
                    }
                }
            }

            if updated_edges.len() > 0 {
                // TODO: When should this be materialized?
                let memobody = MemoBody::Edge(updated_edges);
                let entity_id = parent_head.entity_id().unwrap();

                let head = self.slab.new_memo(Some(entity_id), parent_head, memobody.clone()).to_head();

                self.apply_head(&head).await?;
            }
        }

        //        debug!("COMPACT Before: {:?}, After: {:?}", before, self.stash.concise_contents() );
        Ok(())
    }

    pub async fn is_fully_materialized(&self) -> Result<bool, RetrieveError> {
        for head in self.stash.iter() {
            if !head.is_fully_materialized(&self.slab).await? {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    pub(crate) async fn update_indices(&self, entity_id: EntityId, head: &Head) -> Result<(), WriteError> {
        // TODO: Consider only using the timestamp portion of the Ulid for indexing:
        // let timestamp: u32 = (ulid.0 >> 32) as u32;
        // TODO: think about using beacon clock readings to somehow build the timestamp portion of the ulid rather than wallclock
        //   That way the index hotspots would be locality-specific >_>
        //   RESEARCH: How is this similar / dissimilar to Ron uuids?
        //   RESEARCH: How do we compress the beacon clock reading into 32 bits while still:
        //           * changing relatively slowly for a given slab
        //           * and very likely to be very different from others in the system

        self.root_index()
            .await?
            .insert(self, entity_id.hack_as_u64(), head.clone())
            .await
        // TODO - update
    }

    /// Called by the Slab whenever memos matching one of our subscriptions comes in, or by the Entity when an edit is
    /// made
    pub(crate) async fn apply_head(&self, head: &Head) -> Result<Head, WriteError> {
        // println!("Context.apply_entity_head({}, {:?}) ", entity_id, head.memo_ids() );
        self.stash.apply_head(&self.slab, head).await
    }

    pub async fn get_entity(&self, entity_id: EntityId) -> Result<Option<Entity>, RetrieveError> {
        let root_index = self.root_index().await?;

        match root_index.get(self, entity_id.hack_as_u64()).await? {
            Some(head) => {
                Ok(Some(Entity { id:
                                     head.entity_id()
                                         .ok_or(RetrieveError::InvalidHead(InvalidHead::MissingEntityId))?,
                                 head,
                                 context: self.clone() }))
            },
            None => Ok(None),
        }
    }

    /// Update a given Head with any relevant information to ensure that our consistency model invariants are met
    #[tracing::instrument(level = "info")]
    pub(crate) async fn mut_update_index_head_for_consistency(&self, mut_head: &mut Head) -> Result<bool, RetrieveError> {
        // TODO - think about immutable versions of this

        assert_ne!(mut_head.len(), 0);
        //        if  {
        //            return Err(RetrieveError::InvalidHead(InvalidHead::Empty));
        //        }

        let apply_head = match mut_head.entity_id() {
            Some(entity_id @ EntityId { stype: EntityType::IndexNode,
                            .. }) => self.stash.get_head(entity_id),
            _ => panic!("Can only be called for EntityType::IndexNode heads"),
        };

        let applied = mut_head.mut_apply(&apply_head, &self.slab).await?;

        Ok(applied)
    }

    #[tracing::instrument(level = "info")]
    pub(crate) async fn mut_update_record_head_for_consistency(&self, mut_head: &mut Head) -> Result<bool, RetrieveError> {
        // TODO - think about immutable versions of this

        assert_ne!(mut_head.len(), 0);

        //        if mut_head.len() == 0 {
        //            return Err(RetrieveError::InvalidHead(InvalidHead::Empty));
        //        }

        let apply_head = match mut_head.entity_id() {
            Some(entity_id @ EntityId { stype: EntityType::Record,
                            .. }) => {
                // TODO: figure out a way to noop here in the case that the EntityHead in question
                //       was pulled against a sufficiently identical context stash state.
                //       Perhaps stash edit increment? how can we get this to be really granular?

                match self.root_index().await?.get(&self, entity_id.hack_as_u64()).await? {
                    Some(head) => head,
                    None => return Ok(false),
                }
            },
            _ => panic!("Can only be called for EntityType::Record heads"),
        };

        let applied = mut_head.mut_apply(&apply_head, &self.slab).await?;

        Ok(applied)
    }

    pub(crate) async fn get_entity_from_head(&self, mut head: Head) -> Result<Entity, RetrieveError> {
        self.mut_update_record_head_for_consistency(&mut head).await?;

        if head.entity_id().is_none() {
            panic!("get_entity_from_head - no entity_id for {:?}", head);
        }

        Ok(Entity { id: head.entity_id()
                            .ok_or(RetrieveError::InvalidHead(InvalidHead::MissingEntityId))?,
                    head,
                    context: self.clone() })
    }
}

impl fmt::Debug for Context {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("ContextShared")
           .field("entity_heads", &self.stash.entity_ids())
           .finish()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        slab::{
            EdgeSet,
            EntityId,
            MemoBody,
        },
        Network,
        Slab,
    };

    use std::collections::HashMap;

    #[unbase_test_util::async_test]
    async fn context_basic() {
        let net = Network::create_new_system();
        let slab = Slab::initialize(&net);
        let context = slab.create_context();

        // 4 -> 3 -> 2 -> 1
        let head1 = context.add_test_head(EntityId::index_test(1), vec![]).await;
        let head2 = context.add_test_head(EntityId::index_test(2), vec![head1]).await;
        let head3 = context.add_test_head(EntityId::index_test(3), vec![head2]).await;
        let _head4 = context.add_test_head(EntityId::index_test(4), vec![head3]).await;

        // each preceeding entity should be pruned, leaving us with a fully compacted stash
        assert_eq!(context.stash.concise_contents(), "I4>I3", "Valid contents");
    }

    #[unbase_test_util::async_test]
    async fn context_manual_compaction() {
        let net = Network::create_new_system();
        let slab = Slab::initialize(&net);
        let context = slab.create_context();

        // 4 -> 3 -> 2 -> 1
        let head1 = context.add_test_head(EntityId::index_test(1), vec![]).await;
        let head2 = context.add_test_head(EntityId::index_test(2), vec![head1]).await;

        {
            // manually defeat compaction
            let head = slab.new_memo(head2.entity_id(), head2.clone(), MemoBody::Edit(HashMap::new()))
                           .to_head();
            context.apply_head(&head).await.unwrap();
        }

        // additional stuff on I2 should prevent it from being pruned by the I3 edge
        let head3 = context.add_test_head(EntityId::index_test(3), vec![head2.clone()]).await;
        let head4 = context.add_test_head(EntityId::index_test(4), vec![head3.clone()]).await;

        assert_eq!(context.stash.concise_contents(), "I2>I1;I4>I3", "Valid contents");

        {
            // manually perform compaction
            let updated_head2 = context.stash.get_head(head2.entity_id().unwrap());
            let head = slab.new_memo(head3.entity_id(),
                                     head3.clone(),
                                     MemoBody::Edge(EdgeSet::single(0, updated_head2)))
                           .to_head();
            context.apply_head(&head).await.unwrap();
        }

        assert_eq!(context.stash.concise_contents(), "I3>I2;I4>I3", "Valid contents");

        {
            // manually perform compaction
            let updated_head3 = context.stash.get_head(head3.entity_id().unwrap());
            let head = slab.new_memo(head4.entity_id(), head4, MemoBody::Edge(EdgeSet::single(0, updated_head3)))
                           .to_head();
            context.apply_head(&head).await.unwrap();
        }

        assert_eq!(context.stash.concise_contents(), "I4>I3", "Valid contents");
    }

    #[unbase_test_util::async_test]
    async fn context_auto_compaction() {
        let net = Network::create_new_system();
        let slab = Slab::initialize(&net);
        let context = slab.create_context();

        // 4 -> 3 -> 2 -> 1
        let head1 = context.add_test_head(EntityId::index_test(1), vec![]).await;

        // head1 gets pruned because it's included as an edge here, and there are no newer edits for it in the stash
        let head2 = context.add_test_head(EntityId::index_test(2), vec![head1]).await;

        {
            // defeat stash pruning during apply_head(head3) by adding an update to the head which is newer than its
            // head2 edge
            let head = slab.new_memo(head2.entity_id(), head2.clone(), MemoBody::Edit(HashMap::new()))
                           .to_head();
            context.apply_head(&head).await.unwrap();
        }

        // additional edit on I2 present in context, but not in this edge should prevent it from being pruned by the
        // apply_head(head3)
        let head3 = context.add_test_head(EntityId::index_test(3), vec![head2]).await;

        {
            // defeat stash pruning during apply_head(head4) by adding an update to the head which is newer than its
            // head3 edge
            let head = slab.new_memo(head3.entity_id(), head3.clone(), MemoBody::Edit(HashMap::new()))
                           .to_head();
            context.apply_head(&head).await.unwrap();
        }

        // additional edit on I3 present in context, but not in this edge should prevent it from being pruned during the
        // apply_head(head4)
        let _head4 = context.add_test_head(EntityId::index_test(4), vec![head3]).await;

        assert_eq!(context.stash.concise_contents(), "I2>I1;I3>I2;I4>I3", "Valid contents");

        context.compact().await.unwrap();

        assert_eq!(context.stash.concise_contents(), "I4>I3", "Valid contents");
    }

    // TODO POSTMERGE - restore these tests

    // #[unbase_test_util::async_test]
    // async fn context_manager_dual_indegree_zero() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = slab.create_context();

    //     // 2 -> 1, 4 -> 3
    //     let head1 = context.add_test_entity(1, None, &slab        );
    //     let head2 = context.add_test_entity(2, Some(1), &slab );
    //     let head3 = context.add_test_entity(3, None,        &slab );
    //     let head4 = context.add_test_entity(4, Some(3), &slab );

    //     let mut iter = context.entity_head_iter();
    //     assert!(iter.get_entity_ids() == [1,3,2,4], "Valid sequence");
    // }
    // #[unbase_test_util::async_test]
    // async fn repoint_relation() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = slab.create_context();

    //     // 2 -> 1, 4 -> 3
    //     // Then:
    //     // 2 -> 4

    //     let head1 = context.add_test_entity(1, None, &slab        );
    //     let head2 = context.add_test_entity(2, Some(1), &slab );
    //     let head3 = context.add_test_entity(3, None,        &slab );
    //     let head4 = context.add_test_entity(4, Some(3), &slab );

    //     // Repoint Entity 2 slot 0 to entity 4
    //     let head2_b = slab.new_memo(Some(2), head2, MemoBody::Relation(RelationSet::single(0,4) )).to_head();
    //     context.apply_head(4, &head2_b, &slab).await.unwrap();

    //     let mut iter = context.entity_head_iter();
    //     assert!(iter.get_entity_ids() == [1,4,3,2], "Valid sequence");
    // }

    // it doesn't actually make any sense to "remove" a head from the context
    // #[unbase_test_util::async_test]
    // async fn context_remove() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = slab.create_context();

    //     // Entity 1 is pointing to nooobody
    //     let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r:
    // RelationSet::empty() }).to_head();     context.apply_head(1, head1.project_all_edge_links(&slab),
    // head1.clone());

    //     // Entity 2 slot 0 is pointing to Entity 1
    //     let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r:
    // RelationSet::single(0, 1) }).to_head();     context.apply_head(2, head2.project_all_edge_links(&slab),
    // head2.clone());

    //     //Entity 3 slot 0 is pointing to Entity 2
    //     let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r:
    // RelationSet::single(0, 2) }).to_head();     context.apply_head(3, head3.project_all_edge_links(&slab),
    // head3.clone());

    //     // 2[0] -> 1
    //     // 3[0] -> 2
    //     // Entity 1 should have indirect_references = 2

    //     context.remove_head(2);

    //     let mut iter = context.entity_head_iter();
    //     // for entity_head in iter {
    //     //     println!("{} is {}", entity_head.entity_id, entity_head.indirect_references );
    //     // }
    //     assert_eq!(3, iter.next().expect("iter result 3 should be present").entity_id);
    //     assert_eq!(1, iter.next().expect("iter result 1 should be present").entity_id);
    //     assert!(iter.next().is_none(), "iter should have ended");
    // }
    // #[test]
    // fn context_manager_add_remove_cycle() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = slab.create_context();

    //     // Entity 1 is pointing to nooobody
    //     let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r:
    // RelationSet::empty() }).to_head();     context.apply_head(1, head1.project_all_edge_links(&slab),
    // head1.clone());

    //     assert_eq!(manager.entity_count(), 1);
    //     assert_eq!(manager.entity_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 0);
    //     context.remove_head(1);
    //     assert_eq!(manager.entity_count(), 0);
    //     assert_eq!(manager.entity_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 1);

    //     // Entity 2 slot 0 is pointing to Entity 1
    //     let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r:
    // RelationSet::single(0, 1) }).to_head();     context.apply_head(2, head2.project_all_edge_links(&slab),
    // head2.clone());

    //     assert_eq!(manager.entity_count(), 2);
    //     assert_eq!(manager.entity_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 0);
    //     context.remove_head(2);
    //     assert_eq!(manager.entity_count(), 0);
    //     assert_eq!(manager.entity_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 2);

    //     //Entity 3 slot 0 is pointing to nobody
    //     let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r:
    // RelationSet::empty() }).to_head();     context.apply_head(3, head3.project_all_edge_links(&slab),
    // head3.clone());

    //     assert_eq!(manager.entity_count(), 1);
    //     assert_eq!(manager.entity_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 1);
    //     context.remove_head(3);
    //     assert_eq!(manager.entity_count(), 0);
    //     assert_eq!(manager.entity_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 2);

    //     // Entity 4 slot 0 is pointing to Entity 3
    //     let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r:
    // RelationSet::single(0, 3) }).to_head();     context.apply_head(4, head4.project_all_edge_links(&slab),
    // head4);

    //     assert_eq!(manager.entity_count(), 2);
    //     assert_eq!(manager.entity_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 0);
    //     context.remove_head(4);
    //     assert_eq!(manager.entity_count(), 0);
    //     assert_eq!(manager.entity_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 2);

    //     let mut iter = context.entity_head_iter();
    //     // for entity_head in iter {
    //     //     println!("{} is {}", entity_head.entity_id, entity_head.indirect_references );
    //     // }
    //     assert!(iter.next().is_none(), "iter should have ended");
    // }

    // #[test]
    // fn context_manager_contention() {

    //     use std::thread;
    //     use std::sync::{Arc,Mutex};

    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);

    //     let interloper = Arc::new(Mutex::new(1));

    //     let mut manager = ContextManager::new_pathological(Box::new(|caller|{
    //         if caller == "pre_increment".to_string() {
    //             interloper.lock().unwrap();
    //         }
    //     }));

    //     let head1 = context.add_test_entity(1, None,        &slab);    // Entity 1 is pointing to nooobody

    //     let lock = interloper.lock().unwrap();
    //     let t1 = thread::spawn(|| {
    //         // should block at the first pre_increment
    //         let head2 = context.add_test_entity(2, Some(head1), &slab);    // Entity 2 slot 0 is pointing to
    // Entity 1         let head3 = context.add_test_entity(3, Some(head2), &slab);    // Entity 3 slot 0 is
    // pointing to Entity 2     });

    //     context.remove_head(1);
    //     drop(lock);

    //     t1.join();

    //     assert_eq!(manager.contains_entity(1),      true  );
    //     assert_eq!(manager.contains_entity_head(1), false );
    //     assert_eq!(manager.contains_entity_head(2), true  );
    //     assert_eq!(manager.contains_entity_head(3), true  );

    //     // 2[0] -> 1
    //     // 3[0] -> 2
    //     // Entity 1 should have indirect_references = 2

    //     let mut iter = context.entity_head_iter();
    //     // for entity_head in iter {
    //     //     println!("{} is {}", entity_head.entity_id, entity_head.indirect_references );
    //     // }
    //     assert_eq!(2, iter.next().expect("iter result 2 should be present").entity_id);
    //     assert_eq!(3, iter.next().expect("iter result 1 should be present").entity_id);
    //     assert!(iter.next().is_none(), "iter should have ended");
    // }
}
