pub mod memory;
pub mod nihdb;
mod corethread;
pub mod requester;

pub use self::memory::Memory;
pub use self::nihdb::NIHDB;
pub use self::corethread::CoreThread;

use slab::{self, prelude::*};
use subject::SubjectId;
use error::*;

use futures::{Future, sync::{mpsc,oneshot}};

pub trait StorageCore {
    fn slab_id (&self) -> slab::SlabId;
}

pub trait StorageCoreInterface {
    fn get_memo <'a> ( &'a mut self, memoref: MemoRef, allow_remote: bool ) -> Box<Future<Item=Memo, Error=Error>>;
    fn put_memo (&mut self, memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef ) -> Box<Future<Item=MemoRef, Error=Error>>;
    fn put_memoref( &mut self, memo_id: MemoId, subject_id: SubjectId, peerset: MemoPeerSet) -> Box<Future<Item=MemoRef, Error=Error>>;
    fn send_memos ( &mut self, slabrefs: &[SlabRef], memorefs: &[MemoRef] ) -> Box<Future<Item=(), Error=Error>>;
    // TODO: Should be Vec<Option<SlabPresence>>.
    fn get_slab_presence (&mut self, slabrefs: Vec<SlabRef>) -> Box<Future<Item=Vec<SlabPresence>, Error=Error>>;
    fn put_slab_presence (&mut self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>>;
    fn get_peerset (&mut self, memoref: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerSet>, Error=Error>>;
}

pub trait StorageInterfaceClone : StorageCoreInterface {
    fn clone(&self) -> Box<StorageInterfaceClone>;
}

// #[cfg(any(feature="single_threaded", all(target_arch = "wasm32", target_os = "unknown")))]
//pub trait StorageInterface : StorageCoreInterface {}

// #[cfg(not(any(feature="single_threaded", all(target_arch = "wasm32", target_os = "unknown"))))]
//pub trait StorageInterface : StorageCoreInterface + StorageInterfaceClone + Send + Sync {}

#[derive(Clone)]
pub struct StorageRequester{
    tx: mpsc::UnboundedSender<(LocalSlabRequest,oneshot::Sender<Result<LocalSlabResponse,Error>>)>
}

pub enum LocalSlabRequest {
    GetMemo { memoref: MemoRef, allow_remote: bool },
    PutMemo { memo: Memo, peerset: MemoPeerSet, from_slabref: SlabRef },
    SendMemo { to_slabrefs: Vec<SlabRef>, memorefs: Vec<MemoRef> },
    RemotizeMemoIds{ memo_ids: Vec<MemoId> },
    GetSlabPresence { slabrefs: Vec<SlabRef> },
    PutSlabPresence { presence: SlabPresence },
    GetPeerSet { memorefs: Vec<MemoRef>, maybe_dest_slabref: Option<SlabRef> },
}
pub enum LocalSlabResponse {
    GetMemo( Memo ),
    PutMemo ( MemoRef ),
    SendMemo ( () ),
    RemotizeMemoIds( () ),
    GetSlabPresence( Vec<SlabPresence> ),
    PutSlabPresence( () ),
    GetPeerSet( Vec<MemoPeerSet> ),
}

pub enum StorageMemoRetrieval {
    Found(Memo),
    Remote(MemoPeerSet)
}

type LocalSlabRequestAndResponder = (LocalSlabRequest,oneshot::Sender<Result<LocalSlabResponse,Error>>);