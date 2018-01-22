Patterns:

    Arc deref pattern:
        Unbase wishes to have as few opinions as possible about the thread model with which you are using it. As such, we presently assume that a Slab and its surrogates could be accessed from multiple different threads. Because of this, we employ a design pattern where essentially all data are wrapped in Arcs. We wish to make this ergonomic and reasonably future proof however. As such, we utilize a pattern of: FooStruct(Arc<FooStructInner>), where the outer FooStruct is essentially just an Arc newtype with a deref impl yeilding the FooStructInner. Essentially all unbase business logic is implemented against the FooStruct, with the Arc derefs happening automatically. The downside of this is that we're doing rather of a lot of Arc derefs internal to their respective business logic. The hope is that this can be replaced later with a non-refcounted approach using generational GC.

Structs:

Slab - A storage place for Memos and MemoRefs

   Slab {
      eventlooop thread
      memo retrieval channel
      memo deposit channel
      ??
   }

   SlabHandle{   Clonable, weak referencing of the slab, able to talk to the slab, replaces WeakSlab
   }

  Slabs do not perform any projections. They ONLY store Memos, MemoRefs, and notify interested parties

  In theory, any and all mutability in the system should be exclusive to the Slab.
  This is not quite the case at present, as MemoRefs and SlabRefs are (non-topologically) mutable. They could be viewed as surrogates of the owning Slab however, thus permitting their mutation.
  This is kind of murky at present. MemoRefs and SlabRefs should be projections of their relevant peering/presence memos as received by the Slab

SlabRef - Reference to a Slab, regardless of whether it is local or remote
  * SlabRef is presently serialized as a single SlabPresence. It should probably hold several SlabPresences for a given slab, as there may be multiple ways to reach it.

  Presently holds a Transmitter bound to the slab in question for the SlabPresence in question
  QUESTION: Within the local process, should there be only one SlabRef per referenced slab, or per origin/dest slab pair? (Is every SlabRef owned by a Slab? )

Network - Facilitator of local Slabs, Transports
  Your local process should have exactly one Network struct (excluding test cases)
  Presently being used for dispatch of Memos received from remote transports, but this may change.

Transport - Modular channel for conveying of memos between slabs
  Transport Types:
    LocalDirect - Minimal MPSC channels, intended to be extremely fast for local use
    Simulator - Deterministic local transport intended for unit tests and scientific experimentation
    UDP - First proper network transport
    Blackhole - Transport that intentionally looses every memo sent. Intended for development/testing purposes

Transmitter - Actual transmitter of Memos, Child handle of a Transport.
  Each Transmitter is bound to a specific destination Slab.
  To be determined: Is a transmitter also specific to an origin slab, or may they be shared between co-resident slabs?

SlabPresence - The ID, TransportAddress, and expected lifetime of a given Slab
  * Serializable for network transport (somewhat intertwined with SlabRef)

  SlabPresence differs slightly from SlabRef insofar as it does not intend to actually reference a slab, but merely contain it's presence information for a given transport at a given time.

Memo - An immutable message - SubjectId, Parent MemoRefs, Body
  * Serializable for network transport

  Memo Bodies: ( some of which contain SlabRefs or MemoRefs )
    SlabPresence - Advertisement of a given SlabPresence (and it's present root index seed. Likely to be split apart later)
    Relation - Edit one or more relations for a given SubjectId
    Edit - Edit one or more fields for a given SubjectId
    FullyMaterialized - A fully materialized representation of state for a given SubjectId
    PartiallyMaterialized - Reserved for future use
    Peering – Update peering for a (different) Memo to indicate that it is available, tracked, or neither by a given Slab
    MemoRequest - Please send this list of memos to this SlabRef

MemoRef - Reference to a specific Memo, whether remote or local
  * Serializable for network transport

  A MemoRef is immutable from a topological standpoint, but mutable in terms of locale.
  In theory, a MemoRef should be a surrogate / extension of the Slab that manages it.
  As conditions change, a Slab may see fit to "remotize" the Memo referenced by a given MemoRef, and thus the MemoRef would need to be mutated.

  It is essential that MemoRefs provide an efficient way to traverse to the referenced Memo when it's resident.
  Thus, this is not a part of the Slab, but rather a surrogate of the Slab. Ideally each memoRef would actually be a projection of relevant peering memos which reference its target Memo.

MemoRefHead

Context

Index


Serialization Format:
Packet[
    From SlabId,
    To   SlabId,
    PeerList
    Memo
]

 [858993464201,null,{"SlabPresence":{"p":{"slab_id":200,"address":{"UDP":{"address":"127.0.0.1:1337"}},"lifetime":"Unknown"},"r":null}},[]]]
