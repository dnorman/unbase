mod core;

mod basic;
pub use self::basic::Memory;

#[cfg(not(target_arch = "wasm32"))]
mod threaded;

#[cfg(not(target_arch = "wasm32"))]
pub use self::threaded::MemoryThreaded;

