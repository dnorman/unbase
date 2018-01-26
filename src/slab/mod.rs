
mod common_structs;
mod memo;
mod slabref;
mod memoref;
mod counter;
mod handle;
pub mod storage;

pub type SlabId = u32;

pub mod prelude {
    pub use slab::SlabId; // Intentionally omitting trait Slab and MemorySlab from here
    pub use slab::handle::SlabHandle;
    pub use slab::common_structs::*;
    pub use slab::slabref::SlabRef;
    pub use slab::memoref::{MemoRef,MemoRefInner,MemoRefPtr};
    pub use slab::memo::{MemoId,Memo,MemoInner,MemoBody};
    pub use slab::memoref::serde as memoref_serde;
    pub use slab::memo::serde as memo_serde;
}

use {context, network};

pub trait Slab {
    fn get_handle (&self)    -> self::handle::SlabHandle;
    fn get_ref (&self)       -> self::slabref::SlabRef;
    fn get_net (&self)       -> network::Network;
    fn create_context(&self) -> context::Context;
}