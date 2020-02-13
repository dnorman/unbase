pub use self::{
    common_structs::*,
    handle::SlabHandle,
    memo::{
        serde as memo_serde,
        Memo,
        MemoBody,
        MemoId,
        MemoInner,
    },
    memoref::{
        serde as memoref_serde,
        MemoRef,
    },
    slabref::{
        SlabRef,
        SlabRefInner,
    },
};

use crate::{
    context::Context,
    network::Network,
    slab::agent::SlabAgent,
};

use crate::slab::state::SlabState;
use std::{
    ops::Deref,
    sync::Arc,
};
use tracing::info;

pub(crate) mod agent;
mod common_structs;
mod handle;
mod state;

mod memo;
mod memoref;
mod slabref;

// TODO - update internal comparisons to use pointer address rather than reading the actual contents
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Hash)]
pub struct SlabId([u8; 32]);

impl SlabId {
    pub fn dummy() -> Self {
        Self([0u8; 32])
    }

    pub fn base64(&self) -> String {
        use base64::encode;

        // Don't use this too much. It allocs!
        encode(&self.0)
    }

    pub fn short(&self) -> String {
        self.base64()[0..6].to_string()
    }
}

impl std::fmt::Display for SlabId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.base64()[..4])
    }
}

impl std::fmt::Debug for SlabId {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("SlabId").field("", &self.base64()).finish()
    }
}

#[derive(Clone)]
pub struct Slab {
    pub id:           SlabId,
    pub(crate) agent: Arc<SlabAgent>,
    pub(crate) state: SlabState,
    pub(crate) net:   Network,
    pub my_ref:       SlabRef,
    //    dispatch_channel: mpsc::Sender<MemoRef>,
    //    dispatcher: Arc<RemoteHandle<()>>,
    handle:           SlabHandle,
}

impl Deref for Slab {
    type Target = SlabHandle;

    fn deref(&self) -> &SlabHandle {
        &self.handle
    }
}

impl Slab {
    #[tracing::instrument]
    pub fn initialize(net: &Network) -> Slab {
        info!("Initializing new slab...");

        use ed25519_dalek::Keypair;
        use rand::rngs::OsRng;
        use sha2::Sha512;

        info!("Generating ed25519 keypair");

        let mut csprng: OsRng = OsRng::new().unwrap();
        let keypair: Keypair = Keypair::generate::<Sha512, _>(&mut csprng);

        info!("Generating new keypair...");

        use std::convert::TryInto;
        let b: [u8; 32] = keypair.public.as_bytes().clone().try_into().unwrap();
        let slab_id = SlabId(b);

        info!("Slab ID {} created", slab_id);

        let state = SlabState::initialize_new_slab(&slab_id, keypair);

        Self::new(net, slab_id)
    }

    #[tracing::instrument]
    pub fn new(net: &Network, slab_id: SlabId) -> Slab {
        let state = SlabState::open(&slab_id);
        Self::new_with_state(net, slab_id, state)
    }

    pub fn new_with_state(net: &Network, slab_id: SlabId, state: SlabState) -> Slab {
        // TODO: figure out how to reconcile this with the simulator
        // let (dispatch_tx_channel, dispatch_rx_channel) = mpsc::channel::<MemoRef>(10);

        let agent = Arc::new(SlabAgent::new(net, slab_id, state.clone()));

        // let dispatcher: RemoteHandle<()> = crate::util::task::spawn_with_handle(
        //     Self::run_dispatcher( agent.clone(), dispatch_rx_channel )
        // );

        let handle = SlabHandle { my_ref: agent.my_ref.clone(),
                                  net:    net.clone(),
                                  // dispatch_channel: dispatch_tx_channel.clone(),
                                  agent:  agent.clone(), };

        let me = Slab { id: slab_id,
                        // dispatch_channel: dispatch_tx_channel,
                        // dispatcher: Arc::new(dispatcher),
                        net: net.clone(),
                        my_ref: agent.my_ref.clone(),
                        handle,
                        agent,
                        state };

        net.register_local_slab(me.handle()).unwrap();

        net.conditionally_generate_root_index_seed(&me.handle);

        me
    }

    // async fn run_dispatcher(agent: Arc<SlabAgent>, mut dispatch_rx_channel: mpsc::Receiver<MemoRef>) {
    //     while let Some(memoref) = dispatch_rx_channel.next().await {
    //         // TODO POSTMERGE reconcile this with reconstitute_memo
    //         agent.notify_local_subscribers(memoref);
    //     }
    // }
    pub fn handle(&self) -> SlabHandle {
        self.handle.clone()
    }

    pub fn create_context(&self) -> Context {
        Context::new(self.handle())
    }

    fn _memo_durability_score(&self, _memo: &Memo) -> u8 {
        // TODO: devise durability_score algo
        //       Should this number be inflated for memos we don't care about?
        //       Or should that be a separate signal?

        // Proposed factors:
        // Estimated number of copies in the network (my count = count of first order peers + their counts weighted by:
        // uptime?) Present diasporosity ( my diasporosity score = mean peer diasporosity scores weighted by
        // what? )
        0
    }
}

impl Drop for Slab {
    fn drop(&mut self) {
        info!("Slab {} was dropped - Shutting down", self.id);
        self.agent.stop();
        self.net.deregister_local_slab(&self.my_ref);
    }
}

impl std::fmt::Debug for Slab {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Slab")
           .field("slab_id", &self.id)
           .field("agent", &self.agent)
           .finish()
    }
}
