pub mod stash;

use subject::{Subject,SubjectId};
use subjecthandle::SubjectHandle;
use network::Network;
use memorefhead::*;
use error::*;

use std::collections::HashMap;
use futures::channel::mpsc;

use index::IndexFixed;
use slab::Slab;
use slab::prelude::*;
use self::stash::Stash;

use std::rc::{Rc,Weak};
use std::cell::RefCell;
use std::ops::Deref;
use std::fmt;
use futures::Stream;

#[derive(Clone)]
pub struct Context(Rc<ContextInner>);

pub struct ContextInner {
    pub slab: LocalSlabHandle,
    pub root_index: RefCell<Option<Rc<IndexFixed>>>,
    net: Network,
    stash: Stash,
    //pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}

#[derive(Clone)]
pub struct WeakContext{
    inner: Weak<ContextInner>
}

impl Deref for Context {
    type Target = ContextInner;
    fn deref(&self) -> &ContextInner {
        &*self.0
    }
}

// TODO: Explain what a context is here
impl Context{
    pub fn new <T> (slab: &T) -> Self where T: Slab {
        let new_self = Context(
            Rc::new(
                ContextInner{
                    slab: slab.get_handle(),
                    net:  slab.get_net(),
                    root_index: RefCell::new(None),
                    stash: Stash::new()
                }
            )
        );

        new_self.init_index_subscription();

        new_self
    }
    pub fn weak (&self) -> WeakContext {
        WeakContext {
            inner: Rc::downgrade(&self.0)
        }
    }
    pub fn fetch_kv (&self, key: &str, val: &str) -> Result<Option<SubjectHandle>,Error> {
        // TODO implement field-specific indexes
            //if I have an index for that field {
            //    use it
            //} else if I am allowed to scan this index...
        self.root_index()?.scan_kv(self, key, val)
        //}
    }
    pub fn fetch_kv_wait (&self, key: &str, val: &str, ms: u64 ) -> Result<SubjectHandle, Error>{
        use std::time::{Instant,Duration};
        let start = Instant::now();
        let wait = Duration::from_millis(ms);
        use std::thread;

        self.root_index_wait( ms )?;

        loop {
            let elapsed = start.elapsed();
            if elapsed > wait {
                return Err(Error::RetrieveError(RetrieveError::NotFoundByDeadline))
            }

            if let Some(rec) = self.fetch_kv(key, val)? {
                return Ok(rec)
            }

            thread::sleep(Duration::from_millis(50));
        }
    }
    fn init_index_subscription (&self) {

        let receiver: mpsc::Receiver<MemoRefHead> = self.slab.observe_index();

        let weak_self = self.weak();

        // TODO: modularize executor
        tokio::spawn(receiver.for_each(move |mrh|{
            if let Some(ctx) = weak_self.upgrade(){
                if let Err(e) = ctx.apply_head(&mrh){ // TODO2 Add action to apply memoref directly
                    //TODO: Update this to use logger
                    println!("Failed to apply head to context {:?}", e);
                }
            }else{
                return Err(()) // Tell the executor that we're done here
            }
        }));
    }
    pub (crate) fn insert_into_root_index(&self, subject_id: SubjectId, subject: &Subject) -> Result<(),Error> {
        self.root_index()?.insert(self, subject_id.id, subject)
    }

    /// Called by the Slab whenever memos matching one of our subscriptions comes in, or by the Subject when an edit is made
    pub (crate) fn apply_head(&self, head: &MemoRefHead) -> Result<MemoRefHead,Error> {
        // println!("Context.apply_subject_head({}, {:?}) ", subject_id, head.memo_ids() );
        self.stash.apply_head(&self.slab, head)
    }
    pub (crate) fn get_subject(&self, subject_id: SubjectId) -> Result<Option<Subject>, Error> {
        self.root_index()?.get(&self, subject_id.id)
    }
    /// Retrieve a subject for a known MemoRefHead – ususally used for relationship traversal.
    /// Any relevant context will also be applied when reconstituting the relevant subject to ensure that our consistency model invariants are met
    pub (crate) fn get_subject_with_head(&self, mut head: MemoRefHeadMut) -> Result<Subject, Error> {

        if head.is_empty() {
            return Err(Error::RetrieveError(RetrieveError::InvalidMemoRefHead));
        }

        let subject_id = head.subject_id();
        if !subject_id.is_anonymous() {
            head.apply( &self.stash.get_head(subject_id), &self.slab )?;
        }
        
        let subject = Subject::reconstitute(&self, head)?;
        Ok(subject)

    }
    pub (crate) fn get_subject_handle_with_head (&self, head: MemoRefHead)  -> Result<SubjectHandle, Error> {
        Ok(SubjectHandle{
            id: head.subject_id().ok_or( Error::RetrieveError(RetrieveError::InvalidMemoRefHead) )?.clone(),
            subject: self.get_subject_with_head(head)?,
            context: self.clone()
        })
    }
        pub fn get_subject_by_id(&self, subject_id: SubjectId) -> Result<Option<SubjectHandle>, Error> {

        match self.root_index()?.get(&self, subject_id.id)? {
            Some(s) => {
                let sh = SubjectHandle{
                    id: subject_id,
                    subject: s,
                    context: self.clone()
                };

                Ok(Some(sh))
            },
            None => Ok(None)
        }
    }

    pub fn concise_contents (&self) -> Vec<String> {
        self.stash.concise_contents()
    }
    // Magically transport subject heads into another context in the same process.
    // This is a temporary hack for testing purposes until such time as proper context exchange is enabled
    // QUESTION: should context exchanges be happening constantly, but often ignored? or requested? Probably the former,
    //           sent based on an interval and/or compaction ( which would also likely be based on an interval and/or present context size)
    pub fn hack_send_context(&mut self, other: &Self) -> usize {
        self.compact().expect("compact");

        let _from_slabref = self.slab.slabref.clone_for_slab(&other.slab);

        let mut memoref_count = 0;

        for head in self.stash.iter() {
            memoref_count += head.len();
            println!("HACK SEND CONTEXT {} ({:?}) From {} to {}",  head.subject_id().unwrap(), head.memo_ids(), self.slab.slab_id(), other.slab.slab_id() );
            other.apply_head(&head.clone_for_slab(&mut self.slab, &other.slab, false)).expect("apply head");
        }

        memoref_count
    }
    pub fn get_relevant_subject_head(&self, subject_id: SubjectId) -> Result<MemoRefHead, Error> {
        use subject::SubjectType::*;
        match subject_id.stype {
            IndexNode => {
                Ok(self.stash.get_head(subject_id).clone())
            },
            Record => {
                // TODO: figure out a way to noop here in the case that the SubjectHead in question
                //       was pulled against a sufficiently identical context stash state.
                //       Perhaps stash edit increment? how can we get this to be really granular?

                match self.root_index()?.get_head(&self, subject_id.id)? {
                    Some(mrh) => Ok(mrh),
                    None      => Ok(MemoRefHead::Null)
                }
            },
            Anonymous => {
                Ok(MemoRefHead::Null)
            }
        }
    }
    pub fn root_index (&self) -> Result<Rc<IndexFixed>,Error> {

        {
           let rg = self.root_index.read().unwrap();
           if let Some( ref arcindex ) = *rg {
               return Ok( arcindex.clone() )
           }
        };
        
        let seed = self.net.get_root_index_seed(&self.slab);
        if seed.is_some() {
            let index = IndexFixed::new_from_memorefhead(&self, 5, seed);
            let rcindex = Rc::new(index);
            *self.root_index.borrow_mut() = Some(rcindex.clone());

            Ok(rcindex)
        }else{
            Err(Error::RetrieveError(RetrieveError::IndexNotInitialized))
        }

    }
    pub fn root_index_wait (&self, wait: u64) -> Result<Rc<IndexFixed>, Error> {
        use std::time::{Instant,Duration};
        let start = Instant::now();
        let wait = Duration::from_millis(wait);
        use std::thread;

        loop {
            if start.elapsed() > wait{
                return Err(Error::RetrieveError(RetrieveError::NotFoundByDeadline))
            }

            if let Ok(ri) = self.root_index() {
                return Ok(ri);
            };

            thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn get_resident_subject_head(&self, subject_id: SubjectId) -> MemoRefHead {
        self.stash.get_head(subject_id).clone()
    }
    pub fn get_resident_subject_head_memo_ids(&self, subject_id: SubjectId) -> Vec<MemoId> {
        self.get_resident_subject_head(subject_id).memo_ids()
    }
    pub fn cmp(&self, other: &Self) -> bool {
        // stable way:
        &*(self.0) as *const _ != &*(other.0) as *const _

        // unstable way:
        // Arc::ptr_eq(&self.inner,&other.inner)
    }

    pub fn add_test_subject(&self, subject_id: SubjectId, relations: Vec<MemoRefHead>) -> MemoRefHead {

        let mut edgeset = EdgeSet::empty();

        for (slot_id, mrh) in relations.iter().enumerate() {
            if let &MemoRefHead::Subject{..} = mrh {
                edgeset.insert(slot_id as RelationSlotId, mrh.clone())
            }
        }

        let memobody = MemoBody::FullyMaterialized { v: HashMap::new(), e: edgeset, t: subject_id.stype };
        let head = self.slab.new_memo_basic_noparent(subject_id, memobody).to_head();

        self.apply_head(&head).expect("apply head")
    }

    /// Attempt to compress the present query context.
    /// We do this by issuing Relation memos for any subject heads which reference other subject heads presently in the query context.
    /// Then we can remove the now-referenced subject heads, and repeat the process in a topological fashion, confident that these
    /// referenced subject heads will necessarily be included in subsequent projection as a result.
    pub fn compact(&self) -> Result<(), Error>  {
        let before = self.stash.concise_contents();

        //TODO: implement topological MRH iterator for stash
        //      non-topological iteration will yield sub-optimal compaction

        // iterate all heads in the stash
        for parent_mrh in self.stash.iter() {
            let mut updated_edges = EdgeSet::empty();

            for edgelink in parent_mrh.project_occupied_edges(&self.slab)? {
                if let EdgeLink::Occupied{slot_id,head:edge_mrh} = edgelink {

                    if let Some(subject_id) = edge_mrh.subject_id().ok(){
                        if let stash_mrh @ MemoRefHead::Subject{..} = self.stash.get_head(subject_id.clone()) {
                            // looking for cases where the stash is fresher than the edge
                            if stash_mrh.descends_or_contains(&edge_mrh, &self.slab)?{
                                updated_edges.insert( slot_id, stash_mrh );
                            }
                        }
                    }
                }
            }

            if updated_edges.len() > 0 {
                // TODO: When should this be materialized?
                let memobody = MemoBody::Edge(updated_edges);
                let subject_id = parent_mrh.subject_id().unwrap();
                let head = self.slab.new_memo_basic(subject_id.clone(), parent_mrh, memobody.clone()).to_head();
                
                self.apply_head(&head)?;
            }
        }

        println!("COMPACT Before: {:?}, After: {:?}", before, self.stash.concise_contents() );
        Ok(())
    }

    pub fn is_fully_materialized(&self) -> bool {
        unimplemented!();
        // for subject_head in self.manager.subject_head_iter() {
        //     if !subject_head.head.is_fully_materialized(&self.slab) {
        //         return false;
        //     }
        // }
        // return true;

    }
}

impl fmt::Debug for Context {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {

        fmt.debug_struct("ContextShared")
            .field("subject_heads", &self.stash.subject_ids() )
            .finish()
    }
}

impl WeakContext {
    pub fn upgrade (&self) -> Option<Context> {
        match self.inner.upgrade() {
            Some(i) => Some( Context(i) ),
            None    => None
        }
    }
}

#[cfg(test)]
mod test {
    use {Network, Slab};
    use slab::storage::Memory;
    use subject::SubjectId;
    use super::EdgeSet;
    use super::Context;
    use super::MemoBody;

    use std::collections::HashMap;

    #[test]
    fn context_basic() {
        let net = Network::create_new_system();
        let slab = Memory::new(&net);
        let context = Context::new(&slab);

        // 4 -> 3 -> 2 -> 1
        let head1  = context.add_test_subject(SubjectId::index_test(1), vec![]    );
        let head2  = context.add_test_subject(SubjectId::index_test(2), vec![head1] );
        let head3  = context.add_test_subject(SubjectId::index_test(3), vec![head2] );
        let _head4 = context.add_test_subject(SubjectId::index_test(4), vec![head3] );

        // each preceeding subject should be pruned, leaving us with a fully compacted stash
        assert_eq!(context.stash.concise_contents(),["I4>I3"], "Valid contents");
    }

    #[test]
    fn context_manual_compaction() {
        let net = Network::create_new_system();
        let slab = Memory::new(&net);
        let slabhandle = slab.get_handle();
        let context = Context::new(&slab);

        // 4 -> 3 -> 2 -> 1
        let head1  = context.add_test_subject(SubjectId::index_test(1), vec![]   );
        let head2  = context.add_test_subject(SubjectId::index_test(2), vec![head1] );

        {
            // manually defeat compaction
            let head = slabhandle.new_memo_basic(head2.subject_id(), head2.clone(), MemoBody::Edit(HashMap::new())).to_head();
            context.apply_head(&head).unwrap();
        }

        // additional stuff on I2 should prevent it from being pruned by the I3 edge
        let head3  = context.add_test_subject(SubjectId::index_test(3), vec![head2.clone()] );
        let head4 = context.add_test_subject(SubjectId::index_test(4), vec![head3.clone()] );

        assert_eq!(context.stash.concise_contents(),["I2>I1","I4>I3"], "Valid contents");

        {
            // manually perform compaction
            let updated_head2 = context.stash.get_head( head2.subject_id() );
            let head = slabhandle.new_memo_basic(head3.subject_id(), head3.clone(), MemoBody::Edge(EdgeSet::single(0, updated_head2))).to_head();
            context.apply_head(&head).unwrap();
        }

        assert_eq!(context.stash.concise_contents(),["I3>I2", "I4>I3"], "Valid contents");

        {
            // manually perform compaction
            let updated_head3 = context.stash.get_head( head3.subject_id() );
            let head = slabhandle.new_memo_basic(head4.subject_id(), head4, MemoBody::Edge(EdgeSet::single(0, updated_head3))).to_head();
            context.apply_head(&head).unwrap();
        }

        assert_eq!(context.stash.concise_contents(),["I4>I3"], "Valid contents");
    }

    #[test]
    fn context_auto_compaction() {
        let net = Network::create_new_system();
        let slab = Memory::new(&net);
        let slabhandle = slab.get_handle();
        let context = Context::new(&slab);

        // 4 -> 3 -> 2 -> 1
        let head1  = context.add_test_subject(SubjectId::index_test(1), vec![]  );
        let head2  = context.add_test_subject(SubjectId::index_test(2), vec![head1]);

        {
            // manually defeat compaction
            let head = slabhandle.new_memo_basic(head2.subject_id(), head2.clone(), MemoBody::Edit(HashMap::new())).to_head();
            context.apply_head(&head).unwrap();
        }

        // additional stuff on I2 should prevent it from being pruned by the I3 edge
        let head3  = context.add_test_subject(SubjectId::index_test(3), vec![head2] );
        {
            // manually defeat compaction
            let head = slabhandle.new_memo_basic(head3.subject_id(), head3.clone(), MemoBody::Edit(HashMap::new())).to_head();
            context.apply_head(&head).unwrap();
        }

        // additional stuff on I3 should prevent it from being pruned by the I4 edge
        let _head4 = context.add_test_subject(SubjectId::index_test(4), vec![head3] );

        assert_eq!(context.stash.concise_contents(),["I2>I1","I3>I2","I4>I3"], "Valid contents");

        context.compact().unwrap();

        assert_eq!(context.stash.concise_contents(),["I4>I3"], "Valid contents");
    }

    // #[test]
    // fn context_manager_dual_indegree_zero() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = Context::new(&slab);

    //     // 2 -> 1, 4 -> 3
    //     let head1 = context.add_test_subject(1, None, &slab        );
    //     let head2 = context.add_test_subject(2, Some(1), &slab );
    //     let head3 = context.add_test_subject(3, None,        &slab );
    //     let head4 = context.add_test_subject(4, Some(3), &slab );

    //     let mut iter = context.subject_head_iter();
    //     assert!(iter.get_subject_ids() == [1,3,2,4], "Valid sequence");
    // }
    // #[test]
    // fn repoint_relation() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = Context::new(&slab);

    //     // 2 -> 1, 4 -> 3
    //     // Then:
    //     // 2 -> 4
        
    //     let head1 = context.add_test_subject(1, None, &slab        );
    //     let head2 = context.add_test_subject(2, Some(1), &slab );
    //     let head3 = context.add_test_subject(3, None,        &slab );
    //     let head4 = context.add_test_subject(4, Some(3), &slab );

    //     // Repoint Subject 2 slot 0 to subject 4
    //     let head2_b = slab.new_memo_basic(Some(2), head2, MemoBody::Relation(RelationSet::single(0,4) )).to_head();
    //     context.apply_head(4, &head2_b, &slab);

    //     let mut iter = context.subject_head_iter();
    //     assert!(iter.get_subject_ids() == [1,4,3,2], "Valid sequence");
    // }
    // #[test]
    // it doesn't actually make any sense to "remove" a head from the context
    // fn context_remove() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = Context::new(&slab);

    //     // Subject 1 is pointing to nooobody
    //     let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::empty() }).to_head();
    //     context.apply_head(1, head1.project_all_edge_links(&slab), head1.clone());

    //     // Subject 2 slot 0 is pointing to Subject 1
    //     let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::single(0, 1) }).to_head();
    //     context.apply_head(2, head2.project_all_edge_links(&slab), head2.clone());

    //     //Subject 3 slot 0 is pointing to Subject 2
    //     let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::single(0, 2) }).to_head();
    //     context.apply_head(3, head3.project_all_edge_links(&slab), head3.clone());


    //     // 2[0] -> 1
    //     // 3[0] -> 2
    //     // Subject 1 should have indirect_references = 2

    //     context.remove_head(2);
        
    //     let mut iter = context.subject_head_iter();
    //     // for subject_head in iter {
    //     //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
    //     // }
    //     assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
    //     assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
    //     assert!(iter.next().is_none(), "iter should have ended");
    // }
    // #[test]
    // fn context_manager_add_remove_cycle() {
    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);
    //     let mut context = Context::new(&slab);

    //     // Subject 1 is pointing to nooobody
    //     let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::empty() }).to_head();
    //     context.apply_head(1, head1.project_all_edge_links(&slab), head1.clone());

    //     assert_eq!(manager.subject_count(), 1);
    //     assert_eq!(manager.subject_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 0);
    //     context.remove_head(1);
    //     assert_eq!(manager.subject_count(), 0);
    //     assert_eq!(manager.subject_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 1);

    //     // Subject 2 slot 0 is pointing to Subject 1
    //     let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::single(0, 1) }).to_head();
    //     context.apply_head(2, head2.project_all_edge_links(&slab), head2.clone());

    //     assert_eq!(manager.subject_count(), 2);
    //     assert_eq!(manager.subject_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 0);
    //     context.remove_head(2);
    //     assert_eq!(manager.subject_count(), 0);
    //     assert_eq!(manager.subject_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 2);

    //     //Subject 3 slot 0 is pointing to nobody
    //     let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::empty() }).to_head();
    //     context.apply_head(3, head3.project_all_edge_links(&slab), head3.clone());

    //     assert_eq!(manager.subject_count(), 1);
    //     assert_eq!(manager.subject_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 1);
    //     context.remove_head(3);
    //     assert_eq!(manager.subject_count(), 0);
    //     assert_eq!(manager.subject_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 2);

    //     // Subject 4 slot 0 is pointing to Subject 3
    //     let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::single(0, 3) }).to_head();
    //     context.apply_head(4, head4.project_all_edge_links(&slab), head4);

    //     assert_eq!(manager.subject_count(), 2);
    //     assert_eq!(manager.subject_head_count(), 1);
    //     assert_eq!(manager.vacancies(), 0);
    //     context.remove_head(4);
    //     assert_eq!(manager.subject_count(), 0);
    //     assert_eq!(manager.subject_head_count(), 0);
    //     assert_eq!(manager.vacancies(), 2);

    //     let mut iter = context.subject_head_iter();
    //     // for subject_head in iter {
    //     //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
    //     // }
    //     assert!(iter.next().is_none(), "iter should have ended");
    // }

    // #[test]
    // fn context_manager_contention() {

    //     use std::thread;
    //     use std::sync::{Arc,Mutex};

    //     let net = Network::create_new_system();
    //     let slab = Slab::new(&net);

    //     let interloper = Arc::new(Mutex::new(1));

    //     let mut manager = ContextManager::new_pathological(Box::new(|caller|{
    //         if caller == "pre_increment".to_string() {
    //             interloper.lock().unwrap();
    //         }
    //     }));


    //     let head1 = context.add_test_subject(1, None,        &slab);    // Subject 1 is pointing to nooobody

    //     let lock = interloper.lock().unwrap();
    //     let t1 = thread::spawn(|| {
    //         // should block at the first pre_increment
    //         let head2 = context.add_test_subject(2, Some(head1), &slab);    // Subject 2 slot 0 is pointing to Subject 1
    //         let head3 = context.add_test_subject(3, Some(head2), &slab);    // Subject 3 slot 0 is pointing to Subject 2
    //     });

    //     context.remove_head(1);
    //     drop(lock);

    //     t1.join();

    //     assert_eq!(manager.contains_subject(1),      true  );
    //     assert_eq!(manager.contains_subject_head(1), false );
    //     assert_eq!(manager.contains_subject_head(2), true  );
    //     assert_eq!(manager.contains_subject_head(3), true  );


    //     // 2[0] -> 1
    //     // 3[0] -> 2
    //     // Subject 1 should have indirect_references = 2

        
    //     let mut iter = context.subject_head_iter();
    //     // for subject_head in iter {
    //     //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
    //     // }
    //     assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
    //     assert_eq!(3, iter.next().expect("iter result 1 should be present").subject_id);
    //     assert!(iter.next().is_none(), "iter should have ended");
    // }
    
}