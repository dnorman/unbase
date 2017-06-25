mod handle;
pub mod core;

pub use self::handle::ContextHandle;
pub use self::core::ContextCore;

pub struct Context;

/// TODO: Explain what a context is here
impl Context{
    pub fn new(slab: &Slab) -> Context {
        let new_self = ContextHandle(Arc::new(ContextCore::new( slab )));
        new_self
    }
}
