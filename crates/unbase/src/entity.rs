use crate::{
    context::Context,
    error::{
        RetrieveError,
        WriteError,
    },
    head::Head,
    slab::{
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

use futures::channel::mpsc;
use std::{
    collections::HashMap,
    fmt,
};

use tracing::debug;

#[derive(Clone)]
pub struct Entity {
    // TODO - remove the redundancy between id and head.entity_id()
    pub id:             EntityId,
    pub(crate) head:    Head,
    pub(crate) context: Context,
}

/// Entity contains a Context (which is an Arc internally) because it IS an enforcer of consistency, and
/// therefore must use the context. Becasuse Entity contains a Context reference, it *MUST NOT BE STORED*
/// anywhere other than user code, otherwise we will create a cycle and thus a memory leak
impl Entity {
    pub async fn new(context: &Context, vals: HashMap<String, String>) -> Result<Entity, WriteError> {
        let slab: &SlabHandle = &context.slab;
        let id = slab.generate_entity_id(EntityType::Record);

        debug!("Entity({}).new()", id);

        let head = slab.new_memo(Some(id),
                                 Head::Null,
                                 MemoBody::FullyMaterialized { v: vals,
                                                               r: RelationSet::empty(),
                                                               e: EdgeSet::empty(),
                                                               t: id.etype.clone(), })
                       .to_head();

        context.update_indices(id, &head).await?;

        let handle = Entity { id,
                              head,
                              context: context.clone() };

        Ok(handle)
    }

    pub async fn new_blank(context: &Context) -> Result<Entity, WriteError> {
        Self::new(context, HashMap::new()).await
    }

    pub async fn new_with_single_kv(context: &Context, key: &str, value: &str) -> Result<Entity, WriteError> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        Self::new(context, vals).await
    }

    #[tracing::instrument(level = "info")]
    pub async fn get_value(&mut self, key: &str) -> Result<Option<String>, RetrieveError> {
        let copy = self.head.clone();
        let applied = self.context.mut_update_record_head_for_consistency(&mut self.head).await?;
        tracing::info!("called mut_update_record_head_for_consistency. Applied: {:?}\n\tWas {:?}\n\tNow {:?}",
                       applied,
                       copy,
                       self.head);

        self.head.get_value(&self.context.slab, key).await
    }

    pub async fn get_edge(&mut self, key: SlotId) -> Result<Option<Entity>, RetrieveError> {
        self.context.mut_update_record_head_for_consistency(&mut self.head).await?;

        match self.head.get_edge(&self.context.slab, key).await? {
            Some(head) => Ok(Some(self.context.get_entity_from_head(head).await?)),
            None => Ok(None),
        }
    }

    pub async fn get_relation(&mut self, key: SlotId) -> Result<Option<Entity>, RetrieveError> {
        self.context.mut_update_record_head_for_consistency(&mut self.head).await?;

        match self.head.get_relation(&self.context.slab, key).await? {
            Some(rel_entity_id) => self.context.get_entity(rel_entity_id).await,
            None => Ok(None),
        }
    }

    pub async fn set_value(&mut self, key: &str, value: &str) -> Result<(), WriteError> {
        self.head.set_value(&self.context.slab, key, value).await?;

        // Update our indices before returning to ensure that subsequence queries against this context are
        // self-consistent
        self.context.update_indices(self.id, &self.head).await?;

        Ok(())
    }

    pub async fn set_relation(&mut self, key: SlotId, relation: &Self) -> Result<(), WriteError> {
        self.head.set_relation(&self.context.slab, key, &relation.head).await?;

        // Update our indices before returning to ensure that subsequence queries against this context are
        // self-consistent
        self.context.update_indices(self.id, &self.head).await?;

        Ok(())
    }

    pub async fn get_all_memo_ids(&self) -> Result<Vec<MemoId>, RetrieveError> {
        self.head.get_all_memo_ids(self.context.slab.clone()).await
    }

    pub fn observe(&self) -> mpsc::Receiver<Head> {
        let (mut tx, rx) = mpsc::channel(1000);

        // get an initial value, rather than waiting for the value to change?
        tx.try_send(self.head.clone())
          .expect("Haven't implemented queue backpressure yet");

        // BUG HERE? - not applying Head to our head here, but double check as to what we were expecting from indexes
        self.context.slab.observe_entity(self.id, tx);

        rx
    }
}

// TODO POSTMERGE dig into https://docs.rs/futures-signals/0.3.11/futures_signals/tutorial/index.html and think about API
// struct EntityState {
//    entity: Entity,
//}

impl fmt::Debug for Entity {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Entity")
           .field("entity_id", &self.id)
           .field("head", &self.head)
           .finish()
    }
}

impl Drop for Entity {
    fn drop(&mut self) {
        // println!("# Entity({}).drop", &self.id);
        // TODO: send a drop signal to the owning context via channel
        // self.drop_channel.send(self.id);
    }
}
