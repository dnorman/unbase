use std::{
    collections::{
        hash_map::Entry,
        HashMap,
    },
    sync::{
        Arc,
        Mutex,
        RwLock,
    },
};

use tracing::debug;

use crate::{
    buffer::SlabBuf,
    error::{
        Error,
        StorageOpDeclined,
    },
    head::Head,
    network::SlabRef,
    slab::{
        memoref::MemoRefPtr,
        state::{
            SlabState,
            SlabStateBufHelper,
        },
        EdgeSet,
        EntityId,
        EntityType,
        Memo,
        MemoBody,
        MemoId,
        MemoInner,
        MemoPeer,
        MemoPeerList,
        MemoPeeringStatus,
        MemoRef,
        SlabId,
        SlabPresence,
        SlabRefInner,
        TransportLiveness,
    },
    Network,
};
use ed25519_dalek::Keypair;
use futures::channel::mpsc;

pub struct SlabAgent {
    pub state:                SlabState,
    pub(crate) net:           Network,
    pub my_ref:               SlabRef,
    //    pub memo_wait_channels:   Mutex<HashMap<MemoId, Vec<oneshot::Sender<Memo>>>>,
    pub entity_subscriptions: RwLock<HashMap<EntityId, Vec<mpsc::Sender<Head>>>>,
    pub index_subscriptions:  RwLock<Vec<mpsc::Sender<Head>>>,
    slabrefs:                 Mutex<HashMap<SlabId, SlabRef>>,
    pub running:              RwLock<bool>,
    keypair:                  Keypair,
}

/// SlabAgent
impl SlabAgent {
    pub fn new(net: &Network, slab_id: SlabId, state: SlabState) -> Self {
        let keypair = state.get_keypair().expect("uninitialized state");

        let my_ref_inner = SlabRefInner { slab_id:  slab_id.clone(),
                                          channels: RwLock::new(vec![]), };

        let my_ref = SlabRef(Arc::new(my_ref_inner));

        let mut slabrefs = HashMap::new();
        slabrefs.insert(slab_id, my_ref.clone());

        SlabAgent { state,
                    net: net.clone(),
                    my_ref,
                    running: RwLock::new(true),
                    keypair,
                    //                    memo_wait_channels: Mutex::new(HashMap::new()),
                    entity_subscriptions: RwLock::new(HashMap::new()),
                    index_subscriptions: RwLock::new(Vec::new()),
                    slabrefs: Mutex::new(slabrefs) }
    }

    pub(crate) fn stop(&self) {
        let mut running = self.running.write().unwrap();
        *running = false;
    }

    pub(crate) fn get_slabref(&self, slab_id: &SlabId, optional_presence: Option<&[SlabPresence]>) -> Result<SlabRef, Error> {
        // TODO - make this an LRU of some kind
        let mut created = false;

        let slabref = match self.slabrefs.lock().unwrap().entry(*slab_id) {
            Entry::Vacant(o) => {
                // Make a new one
                let inner = SlabRefInner { slab_id:  slab_id.clone(),
                                           channels: RwLock::new(vec![]), };

                let slabref = SlabRef(Arc::new(inner));

                // Cache the slabref so all SlabRefs are clones of each other
                o.insert(slabref.clone());

                created = true;
                slabref
            },
            Entry::Occupied(mut o) => o.get().clone(),
        };

        let mut changed = false;
        if let Some(presence) = optional_presence {
            for p in presence.iter() {
                assert!(*slab_id == p.slab_id, "presence slab_id does not match the provided slab_id");

                if slabref.apply_presence_only(p, &self.net)? {
                    changed = true;
                }
            }
        }

        if created || changed {
            // save it!
            self.state
                .put_slab(&slabref.slab_id, SlabBuf::from_slabref(&slabref, &SlabStateBufHelper {}))?;
        }

        Ok(slabref)
    }

    // Counters,stats, reporting
    #[allow(unused)]
    //    pub fn count_of_memorefs_resident(&self) -> u32 {
    //        let state = self.state.read().unwrap();
    //        state.memorefs_by_id.len() as u32
    //    }
    #[allow(unused)]
    pub fn count_of_memos_received(&self) -> u64 {
        self.state.get_counter(b"memos_received")
    }

    #[allow(unused)]
    pub fn count_of_memos_reduntantly_received(&self) -> u64 {
        self.state.get_counter(b"memos_redundantly_received")
    }

    #[allow(unused)]
    pub fn peer_slab_count(&self) -> usize {
        self.state.slab_count()
    }

    pub(crate) fn is_running(&self) -> bool {
        let running = self.running.read().unwrap();
        *running
    }

    #[tracing::instrument]
    pub fn new_memo(&self, entity_id: Option<EntityId>, parents: Head, body: MemoBody) -> MemoRef {
        // Determine MemoId Lazily
        let memo = Memo::new(MemoInner { owning_slabref: self.my_ref.clone(),
                                         entity_id,
                                         parents,
                                         body });

        let (memoref, _had_memoref) = self.state.put_memo(memo);
        // Skip writing the peer list. We don't have one!
        self.consider_emit_memo(&memoref);

        memoref
    }

    pub fn generate_entity_id(&self, etype: EntityType) -> EntityId {
        EntityId::new(etype)
    }

    pub fn get_memo(&self, memoref: MemoRef) -> Result<Option<Memo>, Error> {
        match self.state.get_memo(memoref)? {
            Some(memobuf) => Ok(Some(memobuf.to_memo(&SlabStateBufHelper {}, &self.my_ref)?)),
            None => Ok(None),
        }
    }

    #[tracing::instrument]
    pub fn consider_emit_memo(&self, memoref: &MemoRef) {
        // Emit memos for durability and notification purposes
        // At present, some memos like peering and slab presence are emitted manually.
        // TODO: This will almost certainly have to change once gossip/plumtree functionality is added

        // TODO: test each memo for durability_score and emit accordingly
        if let Some(memo) = memoref.get_memo_if_resident() {
            let needs_peers = self.check_peering_target(&memo);

            unimplemented!()
            //            debug!("memo is resident");
            //            for peer_ref in state.peer_refs
            //                                 .iter()
            //                                 .filter(|x| !memoref.is_peered_with_slabref(x))
            //                                 .take(needs_peers as usize)
            //            {
            //                peer_ref.send(&self.my_ref, memoref);
            //            }
        }
    }

    fn check_peering_target(&self, memo: &Memo) -> u8 {
        if memo.does_peering() {
            5
        } else {
            // This is necessary to prevent memo routing loops for now, as
            // memoref.is_peered_with_slabref() obviously doesn't work for non-peered memos
            // something here should change when we switch to gossip/plumtree, but
            // I'm not sufficiently clear on that at the time of this writing
            0
        }
    }

    pub fn memo_wait_channel(&self, memo_id: MemoId) -> futures::channel::oneshot::Receiver<Memo> {
        unimplemented!()
        //        let (tx, rx) = futures::channel::oneshot::channel();
        //
        //        // TODO this should be moved to agent
        //        let mut state = self.state.write().unwrap();
        //        match state.memo_wait_channels.entry(memo_id) {
        //            Entry::Vacant(o) => {
        //                o.insert(vec![tx]);
        //            },
        //            Entry::Occupied(mut o) => {
        //                o.get_mut().push(tx);
        //            },
        //        };
        //
        //        rx
    }

    pub fn observe_index(&self, tx: mpsc::Sender<Head>) {
        let mut index_subs = self.index_subscriptions.write().unwrap();
        index_subs.push(tx);
    }

    #[tracing::instrument]
    pub fn check_memo_waiters(&self, memo: &Memo) {
        unimplemented!()
        //        let mut state = self.state.write().unwrap();
        //        match state.memo_wait_channels.entry(memo.id) {
        //            Entry::Occupied(o) => {
        //                let (_, v) = o.remove_entry();
        //                for sender in v {
        //                    // we don't care if it worked or not.
        //                    // if the channel is closed, we're scrubbing it anyway
        //                    sender.send(memo.clone()).ok();
        //                }
        //            },
        //            Entry::Vacant(_) => {},
        //        };
    }

    /// Perform necessary tasks given a newly arrived memo on this slab
    #[tracing::instrument(skip(self), level = "trace")]
    pub fn handle_memo_from_other_slab(&self, memo: &Memo, memoref: &MemoRef, origin_slabref: &SlabRef) -> Result<(), Error> {
        tracing::info!("SlabAgent({})::handle_memo_from_other_slab({:?})", self.my_ref, memo);

        match memo.body {
            // This Memo is a peering status update for another memo
            MemoBody::SlabPresence { p: ref presence,
                                     r: ref root_index_seed, } => {
                match root_index_seed {
                    &Head::Entity { .. } | &Head::Anonymous { .. } => {
                        // HACK - this should be done inside the deserialize
                        for memoref in root_index_seed.iter() {
                            memoref.update_peer(origin_slabref, MemoPeeringStatus::Resident);
                        }

                        self.net.apply_root_index_seed(&presence, root_index_seed, &self.my_ref);
                    },
                    &Head::Null => {},
                }

                let mut reply = false;
                if let &Head::Null = root_index_seed {
                    reply = true;
                }

                if reply {
                    if let Ok(mentioned_slabref) = self.get_slabref(&presence.slab_id, Some(&[*presence])) {
                        // TODO: should we be telling the origin slabref, or the presence slabref that we're here?
                        //       these will usually be the same, but not always

                        let my_presence_memoref =
                            self.new_memo(None,
                                          memoref.to_head(),
                                          MemoBody::SlabPresence { p: self.presence_for_origin(origin_slabref)?,
                                                                   r: self.net.get_root_index_seed_for_agent(&self), });

                        origin_slabref.send(&self.my_ref, &my_presence_memoref);

                        let _ = mentioned_slabref;
                        // needs PartialEq
                        // if mentioned_slabref != origin_slabref {
                        //   mentioned_slabref.send( &self.my_ref, &my_presence_memoref );
                        //}
                    }
                }
            },
            MemoBody::Peering(memo_id, entity_id, ref peerlist) => {
                unimplemented!()
                //                let (peered_memoref, _had_memo) = self.assert_memoref(memo_id, entity_id, peerlist.clone(),
                // None);
                //
                //                // Don't peer with yourself
                //                for peer in peerlist.iter().filter(|p| p.slabref.0.slab_id != self.id) {
                //                    peered_memoref.update_peer(&peer.slabref, peer.status.clone());
                //                }
            },
            MemoBody::MemoRequest(ref desired_memo_ids, ref requesting_slabref) => {
                if *requesting_slabref != self.my_ref {
                    unimplemented!()

                    //                    for desired_memo_id in desired_memo_ids {
                    //                        let maybe_desired_memoref = {
                    //
                    //                            let state = self.state.read().unwrap();
                    //                            match state.memorefs_by_id.get(&desired_memo_id) {
                    //                                Some(mr) => Some(mr.clone()),
                    //                                None => None,
                    //                            }
                    //
                    //                        };
                    //
                    //                        if let Some(desired_memoref) = maybe_desired_memoref {
                    //                            if desired_memoref.is_resident() {
                    //                                requesting_slabref.send(&self.my_ref, &desired_memoref);
                    //                            } else {
                    //                                // Somebody asked me for a memo I don't have
                    //                                // It would be neighborly to tell them I don't have it
                    //                                self.do_peering(&memoref, requesting_slabref);
                    //                            }
                    //                        } else {
                    //                            let peering_memoref = self.new_memo(
                    //                                                                None,
                    //                                                                memoref.to_head(),
                    //                                                                MemoBody::Peering(
                    //                                *desired_memo_id,
                    //                                None,
                    //                                MemoPeerList::new(vec![MemoPeer { slabref: self.my_ref.clone(),
                    //                                                                  status:
                    // MemoPeeringStatus::NonParticipating, }]),                            ),
                    //                            );
                    //                            requesting_slabref.send(&self.my_ref, &peering_memoref)
                    //                        }
                    //                    }
                }
            },
            _ => {},
        }

        Ok(())
    }

    //    pub fn get_memo (&self, memoref: MemoRef ){
    //}
    // should this be a function of the slabref rather than the owning slab?
    pub fn presence_for_origin(&self, origin_slabref: &SlabRef) -> Result<SlabPresence, Error> {
        // Get the address that the remote slab would recogize
        Ok(SlabPresence { slab_id:  self.my_ref.slab_id,
                          address:  origin_slabref.get_return_address()?,
                          liveness: TransportLiveness::Unknown, })
    }

    #[tracing::instrument]
    pub fn do_peering(&self, memoref: &MemoRef, origin_slabref: &SlabRef) {
        let do_send = if let Some(memo) = memoref.get_memo_if_resident() {
            // Peering memos don't get peering memos, but Edit memos do
            // Abstracting this, because there might be more types that don't do peering
            memo.does_peering()
        } else {
            // we're always willing to do peering for non-resident memos
            true
        };

        if do_send {
            // That we received the memo means that the sender didn't think we had it
            // Whether or not we had it already, lets tell them we have it now.
            // It's useful for them to know we have it, and it'll help them STFU

            // TODO: determine if peering memo should:
            //    A. use parents at all
            //    B. and if so, what should be should we be using them for?
            //    C. Should we be sing that to determine the peered memo instead of the payload?

            let peering_memoref = self.new_memo(None,
                                                memoref.to_head(),
                                                MemoBody::Peering(memoref.id,
                                                                  memoref.entity_id,
                                                                  memoref.get_peerlist_for_peer(&self.my_ref,
                                                                                                Some(&origin_slabref.slab_id))));
            origin_slabref.send(&self.my_ref, &peering_memoref);
        }
    }

    pub(crate) fn observe_entity(&self, entity_id: EntityId, tx: mpsc::Sender<Head>) {
        let mut entity_subs = self.entity_subscriptions.write().unwrap();

        match entity_subs.entry(entity_id) {
            Entry::Vacant(e) => {
                e.insert(vec![tx]);
            },
            Entry::Occupied(mut e) => {
                e.get_mut().push(tx);
            },
        };
    }

    #[tracing::instrument]
    pub fn notify_local_subscribers(&self, memoref: MemoRef) {
        let entity_id = match memoref.entity_id {
            Some(entity_id) => entity_id,
            None => return,
        };

        // TODO POSTMERGE - come up with a memoref receiving strategy which allows backpressure when the queue is full
        //        let mut futs = Vec::new();

        {
            let mut index_subs = self.index_subscriptions.write().unwrap();

            if let EntityType::IndexNode = entity_id.etype {
                // TODO3 - update this to consider popularity of this node, and/or common points of reference with a
                // given context selective hearing?

                let senders = &mut index_subs;
                let len = senders.len();

                // TODO POSTMERGE - alright, this approach isn't going to work.
                //    fn send(&mut self, item: Item) -> Send<'_, Self, Item>
                // it returns a Send future which contains &mut self
                // so collecting these futures won't work unless we clone...
                // and maybe not even then, because the clones won't live long enough for &mut self

                for i in (0..len).rev() {
                    let r = { senders[i].try_send(memoref.to_head()) };

                    match r {
                        Ok(_) => {},
                        Err(e) => {
                            // the fact that SendError.kind is private is :facepalm:
                            if e.is_disconnected() {
                                senders.swap_remove(i);
                            } else {
                                panic!("one of the index_subscriptions queues is full, and I haven't implemented async sending \
                                        yet")
                            }
                        },
                    }
                }
            }
        }
        {
            let mut entity_subs = self.entity_subscriptions.write().unwrap();

            if let Some(ref mut senders) = entity_subs.get_mut(&entity_id) {
                let len = senders.len();
                for i in (0..len).rev() {
                    let r = { senders[i].try_send(memoref.to_head()) };

                    match r {
                        Ok(_) => {},
                        Err(e) => {
                            // the fact that SendError.kind is private is :facepalm:
                            if e.is_disconnected() {
                                senders.swap_remove(i);
                            } else {
                                panic!("one of the entity_subscriptions queues is full, and I haven't implemented async sending \
                                        yet")
                            }
                        },
                    }
                }
            }
        }

        //        join_all(futs).await;
    }

    #[tracing::instrument]
    pub fn localize_slabref(&self, slabref: &SlabRef) -> SlabRef {
        // For now, we don't seem to care what slabref we're being cloned from, just which one we point to

        // IF this slabref points to the destination slab, then use to_sab.my_ref
        // because we know it exists already, and we're not allowed to assert a self-ref
        if self.my_ref.slab_id == slabref.slab_id {
            self.my_ref.clone()
        } else {
            // let address = &*self.return_address.read().unwrap();
            // let args = TransmitterArgs::Remote( &self.slab_id, address );

            unimplemented!()
            //            let presence = { slabref.channels.read().unwrap().clone() };
            //            self.assert_slabref(slabref.slab_id, &presence)
        }
    }

    #[tracing::instrument]
    pub fn localize_head(&self, head: &Head, from_slabref: &SlabRef, include_memos: bool) -> Head {
        let local_from_slabref = self.localize_slabref(&from_slabref);

        match head {
            Head::Null => Head::Null,
            Head::Anonymous { ref head, .. } => {
                Head::Anonymous { owning_slabref: self.my_ref.clone(),
                                  head:           head.iter()
                                                      .map(|mr| self.localize_memoref(mr, &local_from_slabref, include_memos))
                                                      .collect(), }
            },
            Head::Entity { entity_id, ref head, .. } => {
                Head::Entity { owning_slabref: self.my_ref.clone(),
                               entity_id:      entity_id.clone(),
                               head:           head.iter()
                                                   .map(|mr| self.localize_memoref(mr, &local_from_slabref, include_memos))
                                                   .collect(), }
            },
        }
    }

    #[tracing::instrument]
    pub fn localize_memoref(&self, memoref: &MemoRef, from_slabref: &SlabRef, include_memo: bool) -> MemoRef {
        // TODO compare SlabRef pointer address rather than id
        if memoref.owning_slabref == self.my_ref {
            return (*memoref).clone();
        }

        //        let peerlist = memoref.get_peerlist_for_peer(from_slabref, Some(self.id));
        //        // TODO - reduce the redundant work here. We're basically asserting the memoref twice
        //        let memoref = self.state.put_memo(memoref.id, memoref.entity_id, peerlist.clone(), match include_memo {
        //                              true => {
        //                                  match *memoref.ptr.read().unwrap() {
        //                                      MemoRefPtr::Resident(ref m) => Some(self.localize_memo(m, from_slabref,
        // &peerlist)),                                      MemoRefPtr::Remote => None,
        //                                  }
        //                              },
        //                              false => None,
        //                          })
        //                          .0;

        //        memoref

        unimplemented!()
    }

    #[tracing::instrument]
    pub fn localize_memo(&self, memo: &Memo, from_slabref: &SlabRef, peerlist: &MemoPeerList) -> Memo {
        unimplemented!()
        //        // TODO - simplify this
        //        self.reconstitute_memo(memo.id,
        //                               memo.entity_id,
        //                               self.localize_head(&memo.parents, from_slabref, false),
        //                               self.localize_memobody(&memo.body, from_slabref),
        //                               from_slabref,
        //                               peerlist)
        //            .0
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub fn reconstitute_memo(&self, memo_id: MemoId, entity_id: Option<EntityId>, parents: Head, body: MemoBody,
                             origin_slabref: &SlabRef, peerlist: &MemoPeerList)
                             -> (Memo, MemoRef, bool) {
        debug!("SlabAgent({})::reconstitute_memo({:?})", self.my_ref, body);

        // TODO: find a way to merge this with assert_memoref to avoid doing duplicative work with regard to peerlist
        // application

        let memo = Memo::new(MemoInner { entity_id,
                                         owning_slabref: self.my_ref.clone(),
                                         parents,
                                         body });

        let (memoref, had_memoref) = self.state.put_memo(memo);
        self.state.put_memopeers(&memoref, peerlist); // TODO - merge these with the ones we might have already had

        self.state.increment_counter(b"memos_received", 1);

        if had_memoref {
            self.state.increment_counter(b"memos_redundantly_received", 1);
        }

        self.consider_emit_memo(&memoref);

        if let Some(ref memo) = memoref.get_memo_if_resident() {
            self.check_memo_waiters(memo);
            // TODO1 - figure out eventual consistency index update behavior. Think fairly hard about blockchain fan-in
            // / block-tree NOTE: this might be a correct place to employ selective hearing. Highest
            // liklihood if the entity is in any of our contexts, otherwise

            self.handle_memo_from_other_slab(memo, &memoref, &origin_slabref).unwrap();
            self.do_peering(&memoref, &origin_slabref);
        }

        self.notify_local_subscribers(memoref.clone());

        // TODO POSTMERGE: reconcile localize_memoref, reconstitute_memo, and recv_memoref
        (memo, memoref, had_memoref)
    }

    #[tracing::instrument]
    fn localize_memobody(&self, mb: &MemoBody, from_slabref: &SlabRef) -> MemoBody {
        match mb {
            &MemoBody::SlabPresence { ref p, ref r } => {
                MemoBody::SlabPresence { p: p.clone(),
                                         r: self.localize_head(r, from_slabref, true), }
            },
            &MemoBody::Relation(ref relationset) => {
                // No slab localization is needed for relationsets
                MemoBody::Relation(relationset.clone())
            },
            &MemoBody::Edge(ref edgeset) => MemoBody::Edge(self.localize_edgeset(edgeset, from_slabref)),
            &MemoBody::Edit(ref hm) => MemoBody::Edit(hm.clone()),
            &MemoBody::FullyMaterialized { ref v,
                                           ref r,
                                           ref t,
                                           ref e, } => {
                MemoBody::FullyMaterialized { v: v.clone(),
                                              r: r.clone(),
                                              e: self.localize_edgeset(e, from_slabref),
                                              t: t.clone(), }
            },
            &MemoBody::PartiallyMaterialized { ref v,
                                               ref r,
                                               ref e,
                                               ref t, } => {
                MemoBody::PartiallyMaterialized { v: v.clone(),
                                                  r: r.clone(),
                                                  e: self.localize_edgeset(e, from_slabref),
                                                  t: t.clone(), }
            },

            &MemoBody::Peering(memo_id, entity_id, ref peerlist) => {
                MemoBody::Peering(memo_id, entity_id, self.localize_peerlist(peerlist))
            },
            &MemoBody::MemoRequest(ref memo_ids, ref slabref) => {
                MemoBody::MemoRequest(memo_ids.clone(), self.localize_slabref(slabref))
            },
        }
    }

    pub fn localize_peerlist(&self, peerlist: &MemoPeerList) -> MemoPeerList {
        MemoPeerList(peerlist.0
                             .iter()
                             .map(|p| {
                                 MemoPeer { slabref: self.localize_slabref(&p.slabref),
                                            status:  p.status.clone(), }
                             })
                             .collect())
    }

    pub fn localize_edgeset(&self, edgeset: &EdgeSet, from_slabref: &SlabRef) -> EdgeSet {
        let new = edgeset.0
                         .iter()
                         .map(|(slot_id, head)| (*slot_id, self.localize_head(head, from_slabref, false)))
                         .collect();

        EdgeSet(new)
    }

    #[allow(unused)]
    #[tracing::instrument]
    pub fn residentize_memoref(&self, memoref: &MemoRef, memo: Memo) -> bool {
        assert!(memoref.owning_slabref == self.my_ref);

        let mut ptr = memoref.ptr.write().unwrap();

        if let MemoRefPtr::Remote = *ptr {
            *ptr = MemoRefPtr::Resident(memo);

            // should this be using do_peering_for_memo?
            // doing it manually for now, because I think we might only want to do
            // a concise update to reflect our peering status change

            let peering_memoref =
                self.new_memo(None,
                              memoref.to_head(),
                              MemoBody::Peering(memoref.id,
                                                memoref.entity_id,
                                                MemoPeerList::new(vec![MemoPeer { slabref: self.my_ref.clone(),
                                                                                  status:  MemoPeeringStatus::Resident, }])));

            for peer in memoref.peerlist.read().unwrap().iter() {
                peer.slabref.send(&self.my_ref, &peering_memoref);
            }

            // residentized
            true
        } else {
            // already resident
            false
        }
    }

    #[allow(unused)]
    #[tracing::instrument]
    pub fn remotize_memoref(&self, memoref: &MemoRef) -> Result<(), StorageOpDeclined> {
        assert!(memoref.owning_slabref == self.my_ref);

        // TODO: check peering minimums here, and punt if we're below threshold

        let send_peers;
        {
            let mut ptr = memoref.ptr.write().unwrap();
            if let MemoRefPtr::Resident(_) = *ptr {
                let peerlist = memoref.peerlist.read().unwrap();

                if peerlist.len() == 0 {
                    return Err(StorageOpDeclined::InsufficientPeering);
                }
                send_peers = peerlist.clone();
                *ptr = MemoRefPtr::Remote;
            } else {
                return Ok(());
            }
        }

        let peering_memoref =
            self.new_memo(None,
                          memoref.to_head(),
                          MemoBody::Peering(memoref.id,
                                            memoref.entity_id,
                                            MemoPeerList::new(vec![MemoPeer { slabref: self.my_ref.clone(),
                                                                              status:  MemoPeeringStatus::Participating, }])));

        // self.consider_emit_memo(&memoref);

        for peer in send_peers.iter() {
            peer.slabref.send(&self.my_ref, &peering_memoref);
        }

        Ok(())
    }

    /// Attempt to remotize the specified memos once. If There is insuffient peering, the storage operation will be
    /// declined immediately
    #[tracing::instrument]
    pub fn try_remotize_memos(&self, memo_ids: &[MemoRef]) -> Result<(), StorageOpDeclined> {
        // TODO accept memoref instead of memoid

        unimplemented!()
        //        let mut memorefs: Vec<MemoRef> = Vec::with_capacity(memo_ids.len());
        //
        //        {
        //            let state = self.state.read().unwrap();
        //            for memo_id in memo_ids.iter() {
        //                if let Some(memoref) = state.memorefs_by_id.get(memo_id) {
        //                    memorefs.push(memoref.clone())
        //                }
        //            }
        //        }
        //
        //        for memoref in memorefs {
        //            self.remotize_memoref(&memoref)?;
        //        }
        //
        //        Ok(())
    }
}

impl std::fmt::Debug for SlabAgent {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        fmt.debug_struct("Slab").field("", &self.my_ref).finish()
    }
}
