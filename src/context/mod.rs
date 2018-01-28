mod internal;
pub mod stash;
mod interface;

use subject::{Subject,SubjectId,SubjectType};
use subjecthandle::SubjectHandle;
use network::Network;
use memorefhead::*;
use error::*;

use std::collections::HashMap;
use futures::sync::mpsc::channel;
use std::thread;

use index::IndexFixed;
use slab::Slab;
use slab::prelude::*;
use self::stash::Stash;

use std::sync::{Arc,Weak,RwLock};
use std::ops::Deref;

#[derive(Clone)]
pub struct Context(Arc<ContextInner>);

pub struct ContextInner {
    pub slab: LocalSlabHandle,
    pub root_index: RwLock<Option<Arc<IndexFixed>>>,
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
            Arc::new(
                ContextInner{
                    slab: slab.get_handle(),
                    net:  slab.get_net(),
                    root_index: RwLock::new(None),
                    stash: Stash::new()
                }
            )
        );

        new_self.init_index_subscription();

        new_self
    }
    pub fn weak (&self) -> WeakContext {
        WeakContext {
            inner: Arc::downgrade(&self.0)
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

        let (tx, rx) = channel(1);
        self.slab.observe_index( tx );

        use futures::Stream;
        let rx = Box::new(rx);
        let weak_self = self.weak();

        // TODO3 - should we be storing the join handle?
        thread::spawn(move || {
            for mr in rx.wait() {
                match mr {
                    Ok(mr) => {
                        if let Some(ctx) = weak_self.upgrade(){
                            if let Err(e) = ctx.apply_head(&mr){ // TODO2 Add action to apply memoref directly
                                //TODO: Update this to use logger
                                println!("Failed to apply head to context {:?}", e);
                            }
                        }
                    },
                    Err(_) => {
                        break;
                    }
                }
            }
        });

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
            let updated_head2 = context.stash.get_head( head2.subject_id().unwrap() );
            let head = slabhandle.new_memo_basic(head3.subject_id(), head3.clone(), MemoBody::Edge(EdgeSet::single(0, updated_head2))).to_head();
            context.apply_head(&head).unwrap();
        }

        assert_eq!(context.stash.concise_contents(),["I3>I2", "I4>I3"], "Valid contents");

        {
            // manually perform compaction
            let updated_head3 = context.stash.get_head( head3.subject_id().unwrap() );
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