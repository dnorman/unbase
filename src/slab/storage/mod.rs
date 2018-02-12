pub mod memory;
pub mod nihdb;
mod corethread;
pub mod requester;

pub use self::memory::Memory;
pub use self::nihdb::NIHDB;
pub use self::corethread::CoreThread;

use slab::prelude::*;
use error::*;

use futures::{Future, sync::{mpsc,oneshot}};

pub trait StorageInterfaceCore {
    fn get_memo ( &self, memoref: MemoRef ) -> Box<Future<Item=Option<Memo>, Error=Error>>;
    fn put_memo (&self, memo: Memo, peerstate: Vec<MemoPeerState>, from_slabref: SlabRef ) -> Box<Future<Item=(), Error=Error>>;
    fn send_memo ( &self, slabref: SlabRef, memoref: MemoRef ) -> Box<Future<Item=(), Error=Error>>;
    fn put_slab_presence (&self, presence: SlabPresence ) -> Box<Future<Item=(), Error=Error>>;
    fn get_peerstate (&self, memoref: MemoRef, maybe_dest_slabref: Option<SlabRef>) -> Box<Future<Item=Vec<MemoPeerState>, Error=Error>>;
}

// pub trait StorageInterfaceClone {
//     fn clone(&self) -> Box<StorageInterfaceClone>;
// }

// #[cfg(any(feature="single_threaded", all(target_arch = "wasm32", target_os = "unknown")))]
// pub trait StorageInterface : StorageInterfaceCore {}

// #[cfg(not(any(feature="single_threaded", all(target_arch = "wasm32", target_os = "unknown"))))]
// pub trait StorageInterface : StorageInterfaceCore + StorageInterfaceClone + Send + Sync {}

#[derive(Clone)]
pub struct StorageRequester{
    tx: mpsc::UnboundedSender<(LocalSlabRequest,oneshot::Sender<Result<LocalSlabResponse,Error>>)>
}

pub enum LocalSlabRequest {
    GetMemo { memoref: MemoRef },
    PutMemo { memo: Memo, peerstate: Vec<MemoPeerState>, from_slabref: SlabRef },
    SendMemo { to_slabref: SlabRef, memoref: MemoRef },
    RemotizeMemoIds{ memo_ids: Vec<MemoId> },
    PutSlabPresence { presence: SlabPresence },
    GetPeerState { memoref: MemoRef, maybe_dest_slabref: Option<SlabRef> },
}
pub enum LocalSlabResponse {
    GetMemo( Option<Memo> ),
    PutMemo ( () ),
    SendMemo ( () ),
    RemotizeMemoIds( () ),
    PutSlabPresence( () ),
    GetPeerState( Vec<MemoPeerState> ),
}