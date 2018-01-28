
    // fn add_receiver (&self, mpsc::UnboundedReceiver<(SlabRequest,oneshot::Sender<SlabResponse>)>);
    // fn put_slabref(&self, slab_id: SlabId, presence: &[SlabPresence] ) -> SlabRef;
    // fn receive_memo_with_peerlist(&self, memo: self::memo::Memo, peerlist: self::common_structs::MemoPeerList, from_slabref: self::slabref::SlabRef ){

    //     let (memoref, had_memoref) = self.assert_memoref(memo.id, memo.subject_id, peerlist.clone(), Some(memo.clone()) );

    //     {
    //         let mut counters = self.counters.write().unwrap();
    //         counters.memos_received += 1;
    //         if had_memoref {
    //             counters.memos_redundantly_received += 1;
    //         }
    //     }
    //     //println!("Slab({}).reconstitute_memo({}) B -> {:?}", self.id, memo_id, memoref );


    //     self.consider_emit_memo(&memoref);

    //     if let Some(ref memo) = memoref.get_memo_if_resident() {

    //         self.check_memo_waiters(memo);
    //         //TODO1 - figure out eventual consistency index update behavior. Think fairly hard about blockchain fan-in / block-tree
    //         // NOTE: this might be a correct place to employ selective hearing. Highest liklihood if the subject is in any of our contexts,
    //         // otherwise 
    //         self.handle_memo_from_other_slab(memo, &memoref, &origin_slabref);
    //         self.do_peering(&memoref, &origin_slabref);

    //     }

    //     self.dispatch_memoref(memoref);


    // }


    /// remove the memo itself from storage, but not the reference to it. Returns Ok(true) if the memo was removed from the store, Ok(false) if it was not present, or Err(_) if there was a problem with removing it
    // fn conditional_remove_memo (&self, memo_id) -> Result<bool,Error> {

    //     if let Some(memoref) = self.by_id.get(memo_id) {

    //         use MemoRefPtr::*;
    //         match memoref.ptr.write().unwrap() {
    //             ptr @ Resident(_) => {
    //                 if memoref.exceeds_min_durability_threshold() {
    //                     *ptr = MemoRefPtr::Remote;

    //                     return Ok(true);
    //                 }else{
    //                     return Err(Error::StorageOpDeclined(StorageOpDeclined::InsufficientPeering));
    //                 }
    //             },
    //             Local  => panic!("Sanity error. Memory Store does not implement MemoRefPtr::Local"),
    //             Remote => return Ok(false);
    //         }

    //     }else{
    //         Ok(false)
    //     }
    // }
    // fn put_memo (&self, memo: Memo) -> Result<(MemoRef, bool), Error> {

    //     let memo = Rc::new(Memo);
    //     self.memos_by_id.insert(memo.id, memo.clone());

    //     self.memorefs_by_id.entry(memo.id) {
    //         Entry::Vacant(o)   => {
    //             let mr = MemoRef(Arc::new(
    //                 MemoRefInner {
    //                     id: memo.id,
    //                     owning_slab_id: self.id,
    //                     subject_id: memo.subject_id,
    //                     peerlist: RwLock::new(vec![]),
    //                     ptr:      RwLock::new( MemoRefPtr::Resident(memo) )
    //                 }
    //             ));

    //             had_memoref = false;
    //             o.insert( mr ).clone()// TODO: figure out how to prolong the borrow here & avoid clone
    //         }
    //         Entry::Occupied(o) => {
    //             let mr = o.get();
    //             had_memoref = true;

    //             let mut ptr = mr.ptr.write().unwrap();
    //             if let MemoRefPtr::Remote = *ptr {
    //                 *ptr = MemoRefPtr::Resident(m)
    //             }
    //             mr.clone()
    //         }
    //     };

    // }
    // fn assert_memoref( &self, memo_id: MemoId, subject_id: Option<SubjectId>, peerlist: MemoPeerList, maybe_memo: Option<Memo>) -> (MemoRef, bool){

    //     match memo {
    //         Some(m) => {
    //             let memo = Rc::new(memo);

    //         }
    //         None => {
                
    //         }
    //     }

    //     let had_memoref;
    //     let memoref = match self.memorefs_by_id.entry(memo_id) {
    //         Entry::Vacant(o)   => {
    //             let mr = MemoRef(Arc::new(
    //                 MemoRefInner {
    //                     id: memo_id,
    //                     owning_slab_id: self.id,
    //                     subject_id: subject_id,
    //                     peerlist: RwLock::new(peerlist),
    //                     ptr:      RwLock::new(match memo {
    //                         Some(m) => {
    //                             assert!(self.id == m.owning_slab_id);
    //                             MemoRefPtr::Resident(m)
    //                         }
    //                         None    => MemoRefPtr::Remote
    //                     })
    //                 }
    //             ));

    //             had_memoref = false;
    //             o.insert( mr ).clone()// TODO: figure out how to prolong the borrow here & avoid clone
    //         }
    //         Entry::Occupied(o) => {
    //             let mr = o.get();
    //             had_memoref = true;
    //             if let Some(m) = memo {

    //                 let mut ptr = mr.ptr.write().unwrap();
    //                 if let MemoRefPtr::Remote = *ptr {
    //                     *ptr = MemoRefPtr::Resident(m)
    //                 }
    //             }
    //             mr.apply_peers( &peerlist );
    //             mr.clone()
    //         }
    //     };

    //     (memoref, had_memoref)
    // }
    // fn fetch_memoref_stream (&self, memo_ids: Box<Stream<Item = &MemoId, Error = ()>>) -> Box<Stream<Item = Option<&MemoRef>, Error = Error>> {
    //     unimplemented!()
    //     //memo_ids.map(|memo_id| self.memorefs_by_id.get(memo_id) )
    // }
    // fn insert_memoref (&self, memoref: MemoRef) {
    //     unimplemented!()
    // }
    // fn fetch_memoref  (&self, memo_id: MemoId) -> Option<MemoRef>{
    //     unimplemented!()
    // }
    // fn insert_memo    (&self, memo: Memo) {
    //     unimplemented!()
    // }
    // fn fetch_memo     (&self, memo_id: MemoId) -> Option<Memo> {
    //     unimplemented!()
    // }


impl MemorySlabWorker {
    pub fn new () -> Self {
        MemorySlabCore {
            id: slab_id,
            counters: counters.clone(),
            handle: handle,

            memo_wait_channels:    Mutex::new(HashMap::new()),
            subject_subscriptions: Mutex::new(HashMap::new()),
            index_subscriptions:   Mutex::new(Vec::new()),
            
            peering_remediation_thread: RwLock::new(None),
            peering_remediation_queue: Mutex::new(Vec::new()),

            my_ref: my_ref,
            peer_refs: RwLock::new(Vec::new()),
            dropping: false
        }

        // TODO: this should really be a thread pool, or get_memo should be changed to be nonblocking somhow
        *me.peering_remediation_thread.write().unwrap() = Some(thread::spawn(move || {
            loop {
                thread::sleep( time::Duration::from_millis(50) );
                //println!("PEERING REMEDIATION");
                if let Some(slab) = weak_self.upgrade(){
                    // TODO - Get rid of this clone. did it as a cheap way to avoid the deadlock below
                    let q = { slab.peering_remediation_queue.lock().unwrap().clone() }; 
                    for memoref in q {
                        // Kind of dumb that it's checking 
                         slab.consider_emit_memo(&memoref);
                    }
                }
            }
        }));


    }

    pub (crate) fn observe_subject (&self, subject_id: SubjectId, tx: futures::sync::mpsc::Sender<MemoRefHead> ) {

        // let (tx,sub) = SubjectSubscription::new( subject_id, self.weak() );

        match self.subject_subscriptions.lock().unwrap().entry(subject_id) {
            Entry::Vacant(e)   => {
                e.insert(vec![tx]);
            },
            Entry::Occupied(mut e) => {
                e.get_mut().push(tx);
            }
        }

        // sub
    }
    pub (crate) fn observe_index (&self, tx: futures::sync::mpsc::Sender<MemoRefHead> ) {
        self.index_subscriptions.lock().unwrap().push(tx);
    }
    // pub fn unsubscribe_subject (&self){
    //     unimplemented!()
    //     // if let Some(subs) = self.subject_subscriptions.lock().unwrap().get_mut(&sub.subject_id) {
    //     //     subs.retain(|s| {
    //     //         s.cmp(&sub)
    //     //     });
    //     // }
    // }
    pub fn memo_wait_channel (&self, memo_id: MemoId ) -> mpsc::Receiver<Memo> {
        let (tx, rx) = channel::<Memo>();

        match self.memo_wait_channels.lock().unwrap().entry(memo_id) {
            Entry::Vacant(o)       => { o.insert( vec![tx] ); }
            Entry::Occupied(mut o) => { o.get_mut().push(tx); }
        };

        rx
    }
    pub fn generate_subject_id(&self, stype: SubjectType) -> SubjectId {
        let mut counters = self.counters.write().unwrap();
        counters.last_subject_id += 1;
        let id = (self.id as u64).rotate_left(32) | counters.last_subject_id as u64;
        SubjectId{ id, stype }
    }
    pub fn check_memo_waiters ( &self, memo: &Memo) {
        match self.memo_wait_channels.lock().unwrap().entry(memo.id) {
            Entry::Occupied(o) => {
                for channel in o.get() {
                    // we don't care if it worked or not.
                    // if the channel is closed, we're scrubbing it anyway
                    channel.send(memo.clone()).ok();
                }
                o.remove();
            },
            Entry::Vacant(_) => {}
        };
    }

    pub fn do_peering(&self, memoref: &MemoRef, origin_slabref: &SlabRef) {

        let do_send = if let Some(memo) = memoref.get_memo_if_resident(){
            // Peering memos don't get peering memos, but Edit memos do
            // Abstracting this, because there might be more types that don't do peering
            memo.does_peering()
        }else{
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
            //println!("MEOW {}, {:?}", my_ref );

            let peering_memoref = self.new_memo(
                None,
                memoref.to_head(),
                MemoBody::Peering(
                    memoref.id,
                    memoref.subject_id,
                    memoref.get_peerlist_for_peer(&self.my_ref, Some(origin_slabref.slab_id))
                )
            );
            origin_slabref.send( &self.my_ref, &peering_memoref );
        }

    }

    pub fn remotize_memo_ids( &self, memo_ids: &[MemoId] ) -> Result<(),Error> { // Stream<Item = &MemoRef, Error = Error>{
        //println!("# Slab({}).remotize_memo_ids({:?})", self.id, memo_ids);

        for memo_id in memo_ids {
            if let Some(memoref) = self.store.get_memoref(memo_id)?{
                self.remotize_memoref(&memoref)?;
            }
        }
        Ok(())

        //let memo_id_stream = futures::stream::iter_ok::<_, ()>(memo_ids.iter());
        //self.store.fetch_memorefs(Box::new(memo_id_stream)).for_each(|mr| { self.remotize_memoref(mr) }).wait()
        // #[async]
        // for &memoref in self.store.fetch_memorefs(memo_ids){
        //     self.remotize_memoref(&memoref)?;
        // }
        // Ok(())
    }
    pub fn new_memo ( &self, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        let memo_id = (self.id as u64).rotate_left(32) | self.counters.last_memo_id() as u64;

        //println!("# Slab({}).new_memo(id: {},subject_id: {:?}, parents: {:?}, body: {:?})", self.id, memo_id, subject_id, parents.memo_ids(), body );

        let memo = Memo::new(MemoInner {
            id:    memo_id,
            owning_slab_id: self.id,
            subject_id: subject_id,
            parents: parents,
            body: body
        });

        let (memoref, _had_memoref) = self.put_memo( memo );
        self.consider_emit_memo(&memoref);

        memoref
    }
    pub fn residentize_memoref(&self, memoref: &MemoRef, memo: Memo) -> bool {
        //println!("# Slab({}).MemoRef({}).residentize()", self.id, memoref.id);

        assert!(memoref.owning_slab_id == self.id);
        assert!( memoref.id == memo.id );

        let mut ptr = memoref.ptr.write().unwrap();

        if let MemoRefPtr::Remote = *ptr {
            *ptr = MemoRefPtr::Resident( memo );

            // should this be using do_peering_for_memo?
            // doing it manually for now, because I think we might only want to do
            // a concise update to reflect our peering status change

            let peering_memoref = self.new_memo(
                None,
                memoref.to_head(),
                MemoBody::Peering(
                    memoref.id,
                    memoref.subject_id,
                    MemoPeerList::new(vec![ MemoPeer{
                        slabref: self.my_ref.clone(),
                        status: MemoPeeringStatus::Resident
                    }])
                )
            );

            for peer in memoref.peerlist.read().unwrap().iter() {
                peer.slabref.send( &self.my_ref, &peering_memoref );
            }

            // residentized
            true
        }else{
            // already resident
            false
        }
    }
    pub fn remotize_memoref( &self, memoref: &MemoRef ) -> Result<(),Error> {
        assert!(memoref.owning_slab_id == self.id);

        //println!("# Slab({}).MemoRef({}).remotize()", self.id, memoref.id );
        
        // TODO: check peering minimums here, and punt if we're below threshold

        self.store.conditional_remove_memo( memoref.id )?;

        let peering_memoref = self.new_memo_basic(
            None,
            memoref.to_head(),
            MemoBody::Peering(
                memoref.id,
                memoref.subject_id,
                MemoPeerList::new(vec![MemoPeer{
                    slabref: self.my_ref.clone(),
                    status: MemoPeeringStatus::Participating
                }])
            )
        );

        //self.consider_emit_memo(&memoref);

        for peer in memoref.peerlist.iter() {
            peer.slabref.send( &self.my_ref, &peering_memoref );
        }

        Ok(())
    }
    pub fn request_memo (&self, memoref: &MemoRef) -> u8 {
        //println!("Slab({}).request_memo({})", self.id, memoref.id );

        let request_memo = self.new_memo_basic(
            None,
            MemoRefHead::Null,
            MemoBody::MemoRequest(
                vec![memoref.id],
                self.my_ref.clone()
            )
        );

        let mut sent = 0u8;
        for peer in memoref.peerlist.read().unwrap().iter().take(5) {
            //println!("Slab({}).request_memo({}) from {}", self.id, memoref.id, peer.slabref.slab_id );
            peer.slabref.send( &self.my_ref, &request_memo.clone() );
            sent += 1;
        }

        sent
    }
    pub fn put_slabref(&self, slab_id: SlabId, presence: &[SlabPresence] ) -> SlabRef {
        //println!("# Slab({}).put_slabref({}, {:?})", self.id, slab_id, presence );

        if slab_id == self.id {
            // Don't even look it up if it's me. We must not allow any third party to edit the peering.
            // Also, my ref won't appear in the list of peer_refs, because it's not a peer
            return self.my_ref.clone();
        }

        let slabref = self.peer_refs.read().expect("peer_refs.read()").entry(slab_id).or_insert_with(|| SlabRef::new( slab_id, self.id ));
        slabref.apply_presence(presence);
        return slabref;
    }
        /// Notify interested parties about a newly arrived memoref on this slab
    pub fn dispatch_memoref (&self, memoref : MemoRef){
        //println!("# \t\\ Slab({}).dispatch_memoref({}, {:?}, {:?})", self.id, &memoref.id, &memoref.subject_id, memoref.get_memo_if_resident() );

        if let Some(subject_id) = memoref.subject_id {
            // TODO: Switch subject subscription mechanism to be be based on channels, and matching trees
            // subject_subscriptions: Mutex<HashMap<SubjectId, Vec<mpsc::Sender<Option<MemoRef>>>>>


            if let SubjectType::IndexNode = subject_id.stype {
                // TODO3 - update this to consider popularity of this node, and/or common points of reference with a given context
                let mut senders = self.index_subscriptions.lock().unwrap();
                let len = senders.len();
                for i in (0..len).rev() {
                    if let Err(_) = senders[i].clone().send(memoref.to_head()).wait(){
                        // TODO3: proactively remove senders when the receiver goes out of scope. Necessary for memory bloat
                        senders.swap_remove(i);
                    }
                }

            }

            if let Some(ref mut senders) = self.subject_subscriptions.lock().unwrap().get_mut( &subject_id ) { 
                let len = senders.len();

                for i in (0..len).rev() {
                    match senders[i].clone().send(memoref.to_head()).wait() {
                        Ok(..) => { }
                        Err(_) => {
                            // TODO3: proactively remove senders when the receiver goes out of scope. Necessary for memory bloat
                            senders.swap_remove(i);
                        }
                    }
                }
            }

        }
    }

    //NOTE: nothing that calls get_memo, directly or indirectly is presently allowed here (but get_memo_if_resident is ok)
    //      why? Presumably due to deadlocks, but this seems sloppy
    /// Perform necessary tasks given a newly arrived memo on this slab
    pub fn handle_memo_from_other_slab( &self, memo: &Memo, memoref: &MemoRef, origin_slabref: &SlabRef ){
        //println!("Slab({}).handle_memo_from_other_slab({:?})", self.id, memo );

        match memo.body {
            // This Memo is a peering status update for another memo
            MemoBody::SlabPresence{ p: ref presence, r: ref root_index_seed } => {

                match root_index_seed {
                    &MemoRefHead::Subject{..} | &MemoRefHead::Anonymous{..} => {
                        // HACK - this should be done inside the deserialize
                        for memoref in root_index_seed.iter() {
                            memoref.update_peer(origin_slabref, MemoPeeringStatus::Resident);
                        }

                        self.net.apply_root_index_seed( &presence, root_index_seed, &self.my_ref );
                    }
                    &MemoRefHead::Null => {}
                }

                let mut reply = false;
                if let &MemoRefHead::Null = root_index_seed {
                    reply = true;
                }

                if reply {
                    if let Ok(mentioned_slabref) = self.slabref_from_presence( presence ) {
                        // TODO: should we be telling the origin slabref, or the presence slabref that we're here?
                        //       these will usually be the same, but not always

                        let my_presence_memoref = self.new_memo_basic(
                            None,
                            memoref.to_head(),
                            MemoBody::SlabPresence{
                                p: self.presence_for_origin( origin_slabref ),
                                r: self.get_root_index_seed()
                            }
                        );

                        origin_slabref.send( &self.my_ref, &my_presence_memoref );

                        let _ = mentioned_slabref;
                        // needs PartialEq
                        //if mentioned_slabref != origin_slabref {
                        //   mentioned_slabref.send( &self.my_ref, &my_presence_memoref );
                        //}
                    }
                }
            }
            MemoBody::Peering(memo_id, subject_id, ref peerlist ) => {
                let (peered_memoref,_had_memo) = self.assert_memoref( memo_id, subject_id, peerlist.clone() );

                // Don't peer with yourself
                for peer in peerlist.iter().filter(|p| p.slabref.0.slab_id != self.id ) {
                    peered_memoref.update_peer( &peer.slabref, peer.status.clone());
                }

                if 0 == peered_memoref.want_peer_count() {
                    self.remove_from_durability_remediation(peered_memoref);
                }
            },
            MemoBody::MemoRequest(ref desired_memo_ids, ref requesting_slabref ) => {

                if requesting_slabref.0.slab_id != self.id {
                    for desired_memo_id in desired_memo_ids {
                        if let Ok(Some(desired_memoref)) = self.store.get_memoref(&desired_memo_id) {

                            if desired_memoref.is_resident() {
                                requesting_slabref.send(&self.my_ref, &desired_memoref)
                            } else {
                                // Somebody asked me for a memo I don't have
                                // It would be neighborly to tell them I don't have it
                                self.do_peering(&memoref,requesting_slabref);
                            }
                        }else{
                            let peering_memoref = self.new_memo(
                                None,
                                memoref.to_head(),
                                MemoBody::Peering(
                                    *desired_memo_id,
                                    None,
                                    MemoPeerList::new(vec![MemoPeer{
                                        slabref: self.my_ref.clone(),
                                        status: MemoPeeringStatus::NonParticipating
                                    }])
                                )
                            );
                            requesting_slabref.send(&self.my_ref, &peering_memoref)
                        }
                    }
                }
            }
            // _ => {
            //     if let Some(SubjectId{stype: SubjectType::IndexNode,..}) = memo.subject_id {
            //         for slab in self.contexts {

            //         }
            //     }
            // }
            _ => {}
        }
    }
    /// Conditionally emit memo for durability assurance
    pub fn consider_emit_memo(&self, memoref: &MemoRef) {
        // At present, some memos like peering and slab presence are emitted manually.
        // TODO: This will almost certainly have to change once gossip/plumtree functionality is added

        let needs_peers = memoref.want_peer_count();

        if needs_peers > 0 {
            self.add_to_durability_remediation(memoref);

            for peer_ref in self.peer_refs.read().unwrap().iter().filter(|x| !memoref.is_peered_with_slabref(x) ).take( needs_peers as usize ) {
                peer_ref.send( &self.my_ref, memoref );
            }
        }else{
            self.remove_from_durability_remediation(&memoref);
        }
    }

    /// Add memorefs which have been deemed as under-replicated to the durability remediation queue
    fn add_to_durability_remediation(&self, memoref: &MemoRef){

        // TODO: transition this to a crossbeam_channel for add/remove and update the remediation thread to manage the list
        let mut rem_q = self.peering_remediation_queue.lock().unwrap();
        if !rem_q.contains(&memoref) {
            rem_q.push(memoref.clone());
        }
    }
    fn remove_from_durability_remediation(&self, memoref: &MemoRef){
        let mut q = self.peering_remediation_queue.lock().unwrap();
        q.retain(|mr| mr != memoref )
    }
    pub fn count_of_memorefs_resident( &self ) -> u32 {
        unimplemented!()
        //self.memorefs_by_id.read().unwrap().len() as u32
    }
    pub fn count_of_memos_received( &self ) -> u64 {
        self.counters.read().unwrap().memos_received as u64
    }
    pub fn count_of_memos_reduntantly_received( &self ) -> u64 {
        self.counters.read().unwrap().memos_redundantly_received as u64
    }
    pub fn peer_slab_count (&self) -> usize {
        self.peer_refs.read().unwrap().len() as usize
    }
        pub fn new_memo_basic (&self, subject_id: Option<SubjectId>, parents: MemoRefHead, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, parents, body)
    }
    pub fn new_memo_basic_noparent (&self, subject_id: Option<SubjectId>, body: MemoBody) -> MemoRef {
        self.new_memo(subject_id, MemoRefHead::Null, body)
    }
    // should this be a function of the slabref rather than the owning slab?
    pub fn presence_for_origin (&self, origin_slabref: &SlabRef ) -> SlabPresence {
        // Get the address that the remote slab would recogize
        SlabPresence {
            slab_id: self.id,
            address: origin_slabref.get_return_address(),
            lifetime: SlabAnticipatedLifetime::Unknown
        }
    }
    pub fn slabref_from_presence(&self, presence: &SlabPresence) -> Result<SlabRef,&str> {
            match presence.address {
                TransportAddress::Simulator  => {
                    return Err("Invalid - Cannot create simulator slabref from presence")
                }
                TransportAddress::Local      => {
                    return Err("Invalid - Cannot create local slabref from presence")
                }
                _ => { }
            };


        //let args = TransmitterArgs::Remote( &presence.slab_id, &presence.address );

        Ok(self.put_slabref( presence.slab_id, &vec![presence.clone()] ))
    }
}

