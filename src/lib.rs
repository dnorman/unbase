//! **unbase** is a causal, coordination-free distributed data-persistence and application framework.
//! It is fundamentally reactive, fault tolerant, and decentralized.
//! It could be thought of as a peer-to-peer database with a causal consistency model,
//! stored procedures and triggers, content-filtered pubsub built in. When Unbase is ready for
//! production use, it should be usable as an application framework, distributing busines logic
//! all around the network as needed.
//!
//! The unbase design entails no server/client distinctions, no masters, no quorums, no DHT, and maximum
//! consistency with human causal expectation. We reject the notion that consistency requires
//! serializability. Orchestration of physical reality doesn't entail centralized arbiters, and
//! neither should our systems.
//!
//! Unbase is presently pre-alpha, and should not yet be used for anything serious.
//! See [unba.se](https://unba.se)for details.
//!
//! - [`Network`](./network/struct.Network.html) Represents an unbase system
//!
//! - [`Slab`](./slab/struct.Slab.html) Storage for constituent elements of the unbase data model:
//! Memos, MemoRefs, and SlabRefs.
//!
//! - [`Context`](./context/struct.Context.html) Enforces the consistency model, allows for queries
//! to be executed
//!
//! - [`Subject`](./subject/struct.Subject.html) Conceptually similar to an Object, or an RDBMS
//! record. Rather than storing state, state is projected as needed to satisfy user queries.
//!
//! ```
//! let net     = unbase::Network::create_new_system(); // use new, except for the very first time
//! let slab    = unbase::Slab::new(&net);
//! let context = slab.create_context();
//!
//! let record  = unbase::SubjectHandle::new_kv(&context, "beast","Tiger").unwrap();
//! let record2 = context.fetch_kv("beast","Tiger").expect("it worked").expect("it was found");
//! record.set_value("sound","Rawwr");
//!
//! assert_eq!(record2.get_value("sound").unwrap(), "Rawwr");
//! ```
#![doc(html_root_url = "https://unba.se")]

#![feature(proc_macro, universal_impl_trait, generators, integer_atomics, use_nested_groups)]
extern crate actix;
extern crate futures;
extern crate core;
extern crate itertools;

#[cfg(not(target_arch = "wasm32"))]
extern crate nihdb;

#[cfg(target_arch = "wasm32")]
//#[macro_use]
extern crate stdweb;

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;


//#[doc(inline)]
mod subject;
pub mod network;
pub mod slab;
pub mod context;
pub mod error;
pub mod index;
pub mod memorefhead;
pub mod util;
pub mod subjecthandle;
pub mod buffer;
pub mod executor;

// #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
// pub mod executor_std;

// #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
// pub mod executor_wasm;

pub use network::Network;
pub use subjecthandle::SubjectHandle;
pub use subject::SubjectId;
pub use slab::Slab;
