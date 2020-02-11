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
//! - [`Entity`](./entity/struct.Entity.html) Conceptually similar to an Object, or an RDBMS
//! record. Rather than storing state, state is projected as needed to satisfy user queries.
//!
//! ```
//! # use unbase::error::RetrieveError;
//! # use unbase::{Network, Slab, Entity};
//! # async fn run () {
//! let net = Network::create_new_system(); // use new, except for the very first time
//! let slab = Slab::initialize(&net); // Slab exits when you drop this
//! let context = slab.create_context(); // Context is your view of the world. A "client" app would have one of these
//!
//! // Lets say one part of the app creates a record
//! let mut original_record = Entity::new_with_single_kv(&context, "beast", "Tiger").await
//!                                                                                 .expect("The record creation didn't fail");
//!
//! // another part of the app happens to be looking for a Tiger record
//! let mut record_copy = context.try_fetch_kv("beast", "Tiger")
//!                              .await
//!                              .expect("the fetch didn't fail")
//!                              .expect("and we found a record");
//!
//! // Now we change the value on the original one
//! original_record.set_value("sound", "Rawwr")
//!                .await
//!                .expect("the set_value didn't fail");
//!
//! // And we can see that the change on the copy
//! assert_eq!(record_copy.get_value("sound").await, Ok(Some("Rawwr".to_string())));
//! # }
//! # async_std::task::block_on(run())
//! ```

#![doc(html_root_url = "https://unba.se")]

#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;

pub mod buffer;
pub mod context;
pub mod entity;
pub mod error;
pub mod head;
pub mod index;
pub mod network;
pub mod slab;
pub mod util;
// pub mod netbuffer;

pub use crate::{
    entity::Entity,
    network::Network,
    slab::Slab,
};
