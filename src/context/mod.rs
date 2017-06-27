mod handle;
pub (crate) mod core;

use slab::*;
pub use self::handle::ContextHandle;
pub use self::core::ContextCore;

use std::sync::Arc;

pub struct Context;

/// TODO: Explain what a context is here
impl Context{
    pub fn new(slab: &Slab) -> ContextHandle {
        ContextHandle{ core: Arc::new(ContextCore::new( slab )) }
    }
}
