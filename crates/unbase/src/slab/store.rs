use std::convert::TryInto;

use crate::{
    buffer::SlabBuf,
    error::StorageError,
    slab::SlabId,
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
pub(crate) struct SlabStore(Arc<SlabStoreInner>);

#[allow(dead_code)]
pub(super) struct SlabStoreInner {
    config:     sled::Tree,
    slabs:      sled::Tree,
    memos:      sled::Tree,
    memo_peers: sled::Tree,
    counters:   sled::Tree,
    slab_id:    SlabId,
}

// TODO - convert this into a trait
impl SlabStore {
    #[allow(unused)]
    pub fn open(basedir: &std::path::Path, slab_id: &SlabId) -> Result<Self, StorageError> {
        let pathbuf = basedir.join(format!("./unbase-{}.sled", slab_id));

        println!("OPEN: {:#?}", pathbuf);
        let db = sled::open(pathbuf.as_path())?;

        let me = Self::new(db, slab_id.clone());

        match me.get_keypair() {
            Ok(_) => Ok(me),
            Err(StorageError::RecordMissing) => Err(StorageError::UninitializedStore),
            Err(e) => Err(e),
        }
    }

    pub(super) fn initialize_new_slab(basedir: &std::path::Path, slab_id: &SlabId, keypair: Keypair)
                                      -> Result<Self, StorageError> {
        let pathbuf = basedir.join(format!("./unbase-{}.sled", slab_id));
        println!("INIT: {:#?}", pathbuf);
        let db = sled::open(pathbuf.as_path())?;

        {
            let config = db.open_tree("config").unwrap();

            config.insert(b"keypair_ed25519", keypair.to_bytes().to_vec()).unwrap();
        }

        Ok(Self::new(db, slab_id.clone()))
    }

    fn new(db: sled::Db, slab_id: SlabId) -> Self {
        let config = db.open_tree("config").unwrap();
        let counters = db.open_tree("counters").unwrap();

        counters.set_merge_operator(merge_counter);

        let slabs = db.open_tree("slabs").unwrap();
        let memos = db.open_tree("memos").unwrap();
        let memo_peers = db.open_tree("memo_peers").unwrap();

        let inner = SlabStoreInner { config,
                                     slabs,
                                     memos,
                                     memo_peers,
                                     counters,
                                     slab_id };

        SlabStore(Arc::new(inner))
    }

    #[allow(dead_code)]
    pub fn slab_id(&self) -> &SlabId {
        &self.0.slab_id
    }

    #[allow(unused)]
    pub(super) fn get_keypair(&self) -> Result<Keypair, StorageError> {
        match self.0.config.get(b"keypair_ed25519").unwrap() {
            Some(b) => Ok(ed25519_dalek::Keypair::from_bytes(&b).unwrap()),
            None => Err(StorageError::RecordMissing),
        }
    }

    #[allow(dead_code)]
    pub fn ident_base64(&self) -> Result<String, StorageError> {
        Ok(base64::encode(self.get_keypair()?.public.as_bytes()))
    }

    #[allow(dead_code)]
    pub fn increment_counter(&self, name: &[u8], increment: u64) -> Result<(), StorageError> {
        self.0.counters.merge(name, &increment.to_ne_bytes())?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn get_counter(&self, name: &[u8]) -> Result<u64, StorageError> {
        match self.0.counters.get(name)? {
            Some(ivec) => Ok(u64::from_ne_bytes((&*ivec as &[u8]).try_into().unwrap())),
            None => Ok(0u64),
        }
    }

    #[allow(unused)]
    pub fn slab_count(&self) -> usize {
        self.0.slabs.len()
    }

    //    #[tracing::instrument]
    //    pub fn put_memo(&self, memo: Memo) -> (MemoRef, bool) {
    //        let buf = MemoBuf::from_memo(memo, &SlabStateBufHelper {});
    //
    //        //        let pb = MemoPeersBuf::<u32, u32> { memo_id: 2,
    //        //            peers:   vec![MemoPeerElement::<u32> { slab_id: 0,
    //        //                status:  MemoPeeringStatus::Resident, }], };
    //        //
    //        self.memos.compare_and_swap(memo_id, None );
    //
    //    }

    //    pub fn get_memo(&self, memoref: MemoRef) -> Result<Option<MemoBuf<EntityId, MemoId, SlabId>>, StorageError> {
    //        // TODO convert this to use a surrogate key, with a separate lookup for MemoId
    //        // A couple reasons for this:
    //        // 1. Save storage space by using a 4 or 8 bytes instead of 256
    //        // 2. De-sparsify the index space
    //        // 3. Enable lazy memo hash calculation, enabling us to defer generation of the actual MemoId until (if/when)
    //        //    we need to send it to another slab
    //
    //        match self.0.memos.get(memoref.id)? {
    //            Some(bytes) => {
    //                let memobuf: MemoBuf<EntityId, MemoId, SlabId> = bincode::deserialize(&bytes[..])?;
    //
    //                Ok(Some(memobuf))
    //            },
    //            None => Ok(None),
    //        }
    //    }

    //    pub fn put_memopeers(&self, memoref: &MemoRef, peers: &MemoPeerList) {
    //        unimplemented!()
    //    }
    //
    pub fn put_slab(&self, slab_id: &SlabId, slabbuf: SlabBuf) -> Result<(), StorageError> {
        let bytes: Vec<u8> = serde_json::to_vec(&slabbuf).unwrap();

        println!("({}) PUT SLAB {}: {}",
                 self.0.slab_id,
                 slab_id,
                 String::from_utf8(bytes.clone()).unwrap());

        self.0.slabs.insert(slab_id.to_be_bytes(), bytes)?;

        Ok(())
    }

    pub fn get_slab(&self, slab_id: &SlabId) -> Result<Option<SlabBuf>, StorageError> {
        println!("({}) GET SLAB {}", self.0.slab_id, slab_id);
        match self.0.slabs.get(slab_id.to_be_bytes())? {
            Some(ivec) => {
                println!("\t Found: {}", String::from_utf8(ivec.to_vec()).unwrap());
                let slabbuf = serde_json::from_slice(&ivec)?;
                Ok(Some(slabbuf))
            },
            None => {
                println!("\tNot found");
                Ok(None)
            },
        }
    }
}

// pub struct SlabStateBufHelper {}
// impl BufferHelper for SlabStateBufHelper {
//    type EntityToken = EntityId;
//    type MemoToken = MemoId;
//    type SlabToken = SlabId;
//
//    fn from_entity_id(&self, entity_id: &EntityId) -> Self::EntityToken {
//        entity_id.clone()
//    }
//
//    fn from_memoref(&self, memoref: &MemoRef) -> Self::MemoToken {
//        memoref.id
//    }
//
//    fn from_slab_id(&self, slab_id: &SlabId) -> Self::SlabToken {
//        slab_id.clone()
//    }
//
//    fn to_entity_id(&self, entity_token: &Self::EntityToken) -> EntityId {
//        entity_token.clone()
//    }
//
//    fn to_memoref(&self, memo_token: &Self::MemoToken) -> MemoRef {
//        unimplemented!()
//        //        memo_token.clone()
//    }
//
//    fn to_slab_id(&self, slab_token: &Self::SlabToken) -> SlabId {
//        slab_token.clone()
//    }
//}

fn merge_counter(_key: &[u8],               // the key being merged
                 last_bytes: Option<&[u8]>, // the previous value, if one existed
                 op_bytes: &[u8]            /* the new bytes being merged in */)
                 -> Option<Vec<u8>> {
    // set the new value, return None to delete

    let old_count = match last_bytes {
        Some(v) => u64::from_ne_bytes(v.try_into().unwrap()),
        None => 0u64,
    };

    let increment = u64::from_ne_bytes(op_bytes.try_into().expect("failed cast"));

    //    println!("{} + {}", old_count, increment);
    let new_value = old_count + increment;
    Some(new_value.to_ne_bytes().to_vec())
}

impl std::fmt::Debug for SlabStore {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        //        use itertools::join;

        fmt.debug_struct("SlabState")
           .field("counters", &self.0.counters)
           // .field( "memorefs_by_id", &(self.memorefs_by_id.keys().join(",")) )
           .finish()
    }
}

impl core::convert::From<sled::Error> for StorageError {
    fn from(error: sled::Error) -> Self {
        StorageError::SledError(error)
    }
}

#[cfg(test)]
mod test {
    use crate::slab::{
        store::SlabStore,
        SlabId,
    };

    use ed25519_dalek::Keypair;
    use rand::rngs::OsRng;
    use sha2::Sha512;

    #[test]
    fn basic_persistence() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path();

        let slab_id = SlabId(1234); // dummy
        let slab_ident;

        {
            let mut csprng: OsRng = OsRng::new().unwrap();
            let keypair: Keypair = Keypair::generate::<Sha512, _>(&mut csprng);
            let store = SlabStore::initialize_new_slab(&tmpdirpath, &slab_id, keypair).unwrap();

            slab_ident = store.ident_base64().unwrap();
            println!("Slab {} initialized", slab_ident);

            assert_eq!(store.get_counter(b"tests_executed").unwrap(), 0);
            store.increment_counter(b"tests_executed", 1).unwrap();
            assert_eq!(store.get_counter(b"tests_executed").unwrap(), 1);
        }

        {
            let store = SlabStore::open(&tmpdirpath, &slab_id).unwrap();

            assert_eq!(slab_ident, store.ident_base64().unwrap(), "slab ident is correct");

            assert_eq!(store.get_counter(b"tests_executed").unwrap(), 1);
            store.increment_counter(b"tests_executed", 1).unwrap();
            assert_eq!(store.get_counter(b"tests_executed").unwrap(), 2);
        }
    }

    #[test]
    fn test_counters() {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path();
        let slab_id = SlabId(4567); // dummy
        let mut csprng: OsRng = OsRng::new().unwrap();
        let keypair: Keypair = Keypair::generate::<Sha512, _>(&mut csprng);
        let store = SlabStore::initialize_new_slab(&tmpdirpath, &slab_id, keypair).unwrap();

        assert_eq!(store.get_counter(b"concurrency_test").unwrap(), 0);

        let a = 1..500; // remember these are half-open ranges.
        let b = 41..540;
        let c = 91..590;

        fn aseq(r: &std::ops::Range<u64>) -> u64 {
            let min = r.start;
            let max = r.end - 1;

            let terms = (r.end - r.start) as f64;
            let avg = (min + max) as f64 / 2.0;

            (terms * avg) as u64
        }

        let expected_total = aseq(&a) + aseq(&b) + aseq(&c);

        crossbeam::scope(|scope| {
            scope.spawn(|_| {
                     for i in a {
                         store.increment_counter(b"concurrency_test", i).unwrap();
                     }
                 });

            scope.spawn(|_| {
                     for i in b {
                         store.increment_counter(b"concurrency_test", i).unwrap();
                     }
                 });

            scope.spawn(|_| {
                     for i in c {
                         store.increment_counter(b"concurrency_test", i).unwrap();
                     }
                 });
        }).unwrap();

        assert_eq!(store.get_counter(b"concurrency_test").unwrap(), expected_total);
    }

    #[allow(dead_code)]
    fn init_test_store() -> SlabStore {
        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path();
        let slab_id = SlabId(4567); // dummy
        let mut csprng: OsRng = OsRng::new().unwrap();
        let keypair: Keypair = Keypair::generate::<Sha512, _>(&mut csprng);
        let store = SlabStore::initialize_new_slab(&tmpdirpath, &slab_id, keypair).unwrap();

        store
    }

    #[test]
    fn store_slabref() {
        //        let store = init_test_store();

        //        let memo = Memo::new(Head::Null, MemoBody::Null });
        //        store.put_memo( )
    }
}
