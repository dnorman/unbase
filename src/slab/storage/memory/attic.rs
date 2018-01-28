
    // fn add_receiver (&self, mpsc::UnboundedReceiver<(SlabRequest,oneshot::Sender<SlabResponse>)>);
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
    pub fn memo_wait_channel (&self, memo_id: MemoId ) -> mpsc::Receiver<Memo> {
        let (tx, rx) = channel::<Memo>();

        match self.memo_wait_channels.lock().unwrap().entry(memo_id) {
            Entry::Vacant(o)       => { o.insert( vec![tx] ); }
            Entry::Occupied(mut o) => { o.get_mut().push(tx); }
        };

        rx
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
}

