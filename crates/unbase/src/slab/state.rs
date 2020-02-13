use std::convert::TryInto;

use crate::{
    buffer::{
        BufferHelper,
        MemoBuf,
        SlabBuf,
    },
    error::{
        Error,
        StorageError,
    },
    slab::{
        EntityId,
        Memo,
        MemoId,
        MemoPeerList,
        MemoRef,
        SlabId,
    },
};
use ed25519_dalek::Keypair;
use std::{
    clone::Clone,
    sync::Arc,
};

/// SlabState stores all state for a slab
/// It may ONLY be owned/touched by SlabAgent. No exceptions.
/// Consider making SlabState a child of SlabAgent to further discourage this
#[derive(Clone)]
pub(super) struct SlabState(Arc<SlabStateInner>);

pub(super) struct SlabStateInner {
    config:     sled::Tree,
    slabs:      sled::Tree,
    memos:      sled::Tree,
    memo_peers: sled::Tree,
    counters:   sled::Tree,
    slab_id:    SlabId,
}

// TODO - convert this into a trait
impl SlabState {
    pub fn open(slab_id: &SlabId) -> Self {
        let path = format!("./unbase-{}.sled", slab_id);
        let db = sled::open(path).unwrap();

        Self::new(db, slab_id.clone())
    }

    pub(super) fn initialize_new_slab(slab_id: &SlabId, keypair: Keypair) -> Self {
        let path = format!("./unbase-{}.sled", slab_id);
        let db = sled::open(path).unwrap();
        {
            let config = db.open_tree("config").unwrap();

            config.insert(b"keypair_ed25519", keypair.to_bytes().to_vec()).unwrap();
        }

        Self::new(db, slab_id.clone())
    }

    fn new(db: sled::Db, slab_id: SlabId) -> Self {
        let config = db.open_tree("config").unwrap();
        let counters = db.open_tree("counters").unwrap();

        counters.set_merge_operator(merge_counter);

        let slabs = db.open_tree("slabs").unwrap();
        let memos = db.open_tree("memos").unwrap();
        let memo_peers = db.open_tree("memo_peers").unwrap();

        let inner = SlabStateInner { config,
                                     slabs,
                                     memos,
                                     memo_peers,
                                     counters,
                                     slab_id };

        SlabState(Arc::new(inner))
    }

    pub(super) fn get_keypair(&self) -> Option<Keypair> {
        match self.0.config.get(b"keypair_ed25519").unwrap() {
            Some(b) => Some(ed25519_dalek::Keypair::from_bytes(&b).unwrap()),
            None => None,
        }
    }

    pub fn increment_counter(&self, name: &[u8], increment: u64) {
        self.0.counters.merge(name, &increment.to_be_bytes());
    }

    pub fn get_counter(&self, name: &[u8]) -> u64 {
        match self.0.counters.get(name).unwrap() {
            Some(ivec) => u64::from_be_bytes((&*ivec as &[u8]).try_into().unwrap()),
            None => 0u64,
        }
    }

    pub fn slab_count(&self) -> usize {
        self.0.slabs.len()
    }

    pub fn get_memo(&self, memoref: MemoRef) -> Result<Option<MemoBuf<EntityId, MemoId, SlabId>>, Error> {
        // TODO convert this to use a surrogate key, with a separate lookup for MemoId
        // A couple reasons for this:
        // 1. Save storage space by using a 4 or 8 bytes instead of 256
        // 2. De-sparsify the index space
        // 3. Enable lazy memo hash calculation, enabling us to defer generation of the actual MemoId until (if/when)
        //    we need to send it to another slab

        match self.0.memos.get(memoref.id)? {
            Some(bytes) => {
                let memobuf: MemoBuf<EntityId, MemoId, SlabId> = bincode::deserialize(&bytes[..])?;

                Ok(Some(memobuf))
            },
            None => Ok(None),
        }
    }

    #[tracing::instrument]
    pub fn put_memo(&self, memo: Memo) -> (MemoRef, bool) {
        let buf = memo.to_buf(self);
        //        let pb = MemoPeersBuf::<u32, u32> { memo_id: 2,
        //            peers:   vec![MemoPeerElement::<u32> { slab_id: 0,
        //                status:  MemoPeeringStatus::Resident, }], };
        //
        //        self.memos.compare_and_swap(memo_id, None );
        //
        //        let had_memoref;
        //
        //
        //
        //        let memoref = match self.state.write().unwrap().memorefs_by_id.entry(memo_id) {
        //            Entry::Vacant(o) => {
        //                let mr = MemoRef(Arc::new(MemoRefInner { id: memo_id,
        //                                                         owning_slab_id: self.id,
        //                                                         entity_id,
        //                                                         peerlist: RwLock::new(peerlist),
        //                                                         ptr: RwLock::new(match memo {
        //                                                                              Some(m) => {
        //                                                                                  assert!(self.id == m.owning_slab_id);
        //                                                                                  MemoRefPtr::Resident(m)
        //                                                                              },
        //                                                                              None => MemoRefPtr::Remote,
        //                                                                          }) }));
        //
        //                had_memoref = false;
        //                o.insert(mr).clone() // TODO: figure out how to prolong the borrow here & avoid clone
        //            },
        //            Entry::Occupied(o) => {
        //                let mr = o.get();
        //                had_memoref = true;
        //                if let Some(m) = memo {
        //                    let mut ptr = mr.ptr.write().unwrap();
        //                    if let MemoRefPtr::Remote = *ptr {
        //                        *ptr = MemoRefPtr::Resident(m)
        //                    }
        //                }
        //                mr.apply_peers(&peerlist);
        //                mr.clone()
        //            },
        //        };
        //
        //        (memoref, had_memoref)
        unimplemented!()
    }

    pub fn put_memopeers(&self, memoref: &MemoRef, peers: &MemoPeerList) {
        unimplemented!()
    }

    pub fn put_slab(&self, slab_id: &SlabId, slabbuf: SlabBuf<EntityId, MemoId>) -> Result<(), Error> {
        let bytes: Vec<u8> = bincode::serialize(&slabbuf).unwrap();

        self.0.slabs.insert(slab_id, bytes)?;

        Ok(())
    }

    pub fn get_slab(&self, slab_id: &SlabId) -> Result<SlabBuf<EntityId, MemoId>, Error> {
        let bytes = self.0.slabs.get(slab_id)?;
        let slabbuf = bincode::deserialize(&bytes[..])?;
        Ok(slabbuf)
    }
}

pub struct SlabStateBufHelper {}
impl BufferHelper for SlabStateBufHelper {
    type EntityToken = EntityId;
    type MemoToken = MemoId;
    type SlabToken = SlabId;

    fn from_entity_id(&self, entity_id: &EntityId) -> Self::EntityToken {
        entity_id.clone()
    }

    fn from_memoref(&self, memoref: &MemoRef) -> Self::MemoToken {
        memoref.id
    }

    fn from_slab_id(&self, slab_id: &SlabId) -> Self::SlabToken {
        slab_id.clone()
    }

    fn to_entity_id(&self, entity_token: &Self::EntityToken) -> EntityId {
        entity_token.clone()
    }

    fn to_memoref(&self, memo_token: &Self::MemoToken) -> MemoRef {
        unimplemented!()
        //        memo_token.clone()
    }

    fn to_slab_id(&self, slab_token: &Self::SlabToken) -> SlabId {
        slab_token.clone()
    }
}

fn merge_counter(_key: &[u8],               // the key being merged
                 last_bytes: Option<&[u8]>, // the previous value, if one existed
                 op_bytes: &[u8]            /* the new bytes being merged in */)
                 -> Option<Vec<u8>> {
    // set the new value, return None to delete

    let old_count = match last_bytes {
        //        let (int_bytes, rest) = input.split_at();
        Some(v) => u64::from_ne_bytes(v.try_into().unwrap()),
        None => 0u64,
    };

    let increment = u64::from_ne_bytes(op_bytes.try_into().expect("failed cast"));

    let new_value = old_count + increment;
    Some(new_value.to_ne_bytes().to_vec())
}

impl std::fmt::Debug for SlabState {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        //        use itertools::join;

        fmt.debug_struct("SlabState")
           .field("counters", &self.counters)
           // .field( "memorefs_by_id", &(self.memorefs_by_id.keys().join(",")) )
           .finish()
    }
}

impl core::convert::From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Error::StorageError(StorageError::Sled(error))
    }
}
