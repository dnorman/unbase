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
        MemoRefInner,
        MemoRefPtr,
    },
    slabref::{
        SlabRef,
        SlabRefInner,
    },
};

use crate::{
    context::Context,
    network::{
        Network,
        Transmitter,
        TransportAddress,
    },
    slab::agent::SlabAgent,
};

use std::{
    ops::Deref,
    sync::{
        Arc,
        Mutex,
        RwLock,
    },
};
use tracing::info;

use crate::slab::store::SlabStore;
use ed25519_dalek::Keypair;
use rand::{
    rngs::OsRng,
    Rng,
};
use sha2::Sha512;
use tempfile::TempDir;

pub(crate) mod agent;
mod common_structs;
mod handle;
mod state;

mod memo;
mod memoref;
mod slabref;
mod store;

pub type SlabId = u32;

#[derive(Clone)]
pub struct Slab {
    pub id:           SlabId,
    pub(crate) agent: Arc<SlabAgent>,
    pub(crate) net:   Network,
    pub my_ref:       SlabRef,
    //    dispatch_channel: mpsc::Sender<MemoRef>,
    //    dispatcher: Arc<RemoteHandle<()>>,
    handle:           SlabHandle,
    tmpdir:           Option<Arc<TempDir>>,
}

impl Deref for Slab {
    type Target = SlabHandle;

    fn deref(&self) -> &SlabHandle {
        &self.handle
    }
}

impl Slab {
    #[tracing::instrument]
    pub fn new_ephemeral(net: &Network) -> Slab {
        let id = net.generate_slab_id();

        let tmpdir = tempfile::tempdir().unwrap();
        let tmpdirpath = tmpdir.path();

        let mut csprng: OsRng = OsRng::new().unwrap();
        let keypair: Keypair = Keypair::generate::<Sha512, _>(&mut csprng);
        let store = SlabStore::initialize_new_slab(&tmpdirpath, &id, keypair).unwrap();

        let agent = Arc::new(SlabAgent::new(net, id.clone(), store));

        // let dispatcher: RemoteHandle<()> = crate::util::task::spawn_with_handle(
        //     Self::run_dispatcher( agent.clone(), dispatch_rx_channel )
        // );

        let handle = SlabHandle { my_ref: agent.my_ref.clone(),
                                  net:    net.clone(),
                                  // dispatch_channel: dispatch_tx_channel.clone(),
                                  agent:  agent.clone(), };

        let me = Slab { id,
                        // dispatch_channel: dispatch_tx_channel,
                        // dispatcher: Arc::new(dispatcher),
                        net: net.clone(),
                        my_ref: agent.my_ref.clone(),
                        handle,
                        agent,
                        tmpdir: Some(Arc::new(tmpdir)) };

        net.register_local_slab(me.handle());

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
        self.net.deregister_local_slab(self.id);
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
