use std::{
    collections::HashMap,
    convert::TryInto,
};

use futures::channel::{
    mpsc,
    oneshot,
};

use crate::{
    head::Head,
    network::SlabRef,
    slab::{
        EntityId,
        Memo,
        MemoId,
        MemoRef,
    },
};
use tracing::info;

/// SlabState stores all state for a slab
/// It may ONLY be owned/touched by SlabAgent. No exceptions.
/// Consider making SlabState a child of SlabAgent to further discourage this
pub(super) struct SlabState {
    config:                   sled::Tree,
    memorefs_by_id:           sled::Tree,
    counters:                 sled::Tree,
    peer_refs:                sled::Tree,
    pub memo_wait_channels:   HashMap<MemoId, Vec<oneshot::Sender<Memo>>>,
    pub entity_subscriptions: HashMap<EntityId, Vec<mpsc::Sender<Head>>>,
    pub index_subscriptions:  Vec<mpsc::Sender<Head>>,
    pub running:              bool,
}

#[derive(Debug)]
pub(crate) struct SlabCounters {
    pub last_memo_id:               u32,
    pub last_entity_id:             u32,
    pub memos_received:             u64,
    pub memos_redundantly_received: u64,
}

// SlabState is forbidden from any blocking operations
// Any code here is holding a mutex lock

impl SlabState {
    pub fn new() -> Self {
        let path = "./unbase.sled";
        let db = sled::open(path).unwrap();

        let config = db.open_tree("config").unwrap();
        let counters = db.open_tree("counters").unwrap();

        counters.set_merge_operator(merge_counter);

        let memorefs_by_id = db.open_tree("memorefs_by_id").unwrap();
        let peer_refs = db.open_tree("peer_refs").unwrap();

        let keypair = match config.get(b"keypair_ed25519").unwrap() {
            Some(b) => ed25519_dalek::Keypair::from_bytes(&b).unwrap(),
            None => {
                // TODO - move this
                use ed25519_dalek::{
                    Keypair,
//                    Signature,
                };
                use rand::{
                    rngs::OsRng,
                    Rng,
                };
                use sha2::Sha512;

                info!("Generating ed25519 keypair");

                let mut csprng: OsRng = OsRng::new().unwrap();
                let keypair: Keypair = Keypair::generate::<Sha512, _>(&mut csprng);

                config.compare_and_swap(b"keypair_ed25519", None, Some(keypair.to_bytes()));

                keypair
            },
        };

        info!("Slab ID {}", keypair.public());

        // TODO - convert this into a trait
        SlabState { config,
                    memorefs_by_id,
                    counters,
                    peer_refs,
                    keypair,
                    running: true,

                    // TODO - move these
                    memo_wait_channels: HashMap::new(),
                    entity_subscriptions: HashMap::new(),
                    index_subscriptions: Vec::new() }
    }

    pub fn increment_counter(&self, name: &[u8], increment: u64) {
        self.counters.merge(name, &increment);
    }

    pub fn get_counter(&self, name: &[u8]) -> u64 {
        match self.counters.get(name).unwrap() {
            Some(bytes) => u64::from_ne_bytes(bytes.try_into().unwrap()),
            None => 0u64,
        }
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
