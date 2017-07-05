mod internal;
mod stash;
mod interface;


use slab::{RelationSlotId,EdgeLink};
use subject::{Subject,SubjectCore,SubjectId,SubjectType};
use memorefhead::*;
use error::*;

use std::collections::HashMap;

use index::IndexFixed;
use slab::*;
use self::stash::Stash;

use std::sync::{Arc,Mutex,RwLock};
use std::ops::Deref;

#[derive(Clone)]
pub struct Context(Arc<ContextInner>);

pub struct ContextInner {
    pub slab: Slab,
    pub root_index: RwLock<Option<IndexFixed>>,
    stash: Stash,
    //pathology:  Option<Box<Fn(String)>> // Something is wrong here, causing compile to fail with a recursion error
}

impl Deref for Context {
    type Target = ContextInner;
    fn deref(&self) -> &ContextInner {
        &*self.0
    }
}

/// TODO: Explain what a context is here
impl Context{
    pub fn new (slab: &Slab) -> Self {
        let new_self = Context(
            Arc::new(
                ContextInner{
                    slab: slab.clone(),
                    root_index: RwLock::new(None),
                    stash: Stash::new()
                }
            )
        );

        let seed = slab.get_root_index_seed().expect("Uninitialized slab");
        let index = IndexFixed::new_from_memorefhead(&new_self, 5, seed);

        *new_self.root_index.write().unwrap() = Some(index);
        new_self
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;
    use {Network, Slab};
    use slab::{MemoBody, RelationSet, EdgeSet};
    use super::Context;

    #[test]
    fn context_basic() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut context = Context::new(&slab);

        // 4 -> 3 -> 2 -> 1
        let head1 = context.add_test_subject(1, None, &slab        );
        let head2 = context.add_test_subject(2, Some(head1), &slab );
        let head3 = context.add_test_subject(3, Some(head2), &slab );
        let head4 = context.add_test_subject(4, Some(head3), &slab );


        let mut iter = context.subject_head_iter();
        assert!(iter.get_subject_ids() == [1,2,3,4], "Valid sequence");
    }

    #[test]
    fn context_manager_dual_indegree_zero() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut context = Context::new(&slab);

        // 2 -> 1, 4 -> 3
        let head1 = context.add_test_subject(1, None, &slab        );
        let head2 = context.add_test_subject(2, Some(head1), &slab );
        let head3 = context.add_test_subject(3, None,        &slab );
        let head4 = context.add_test_subject(4, Some(head3), &slab );

        let mut iter = context.subject_head_iter();
        assert!(iter.get_subject_ids() == [1,3,2,4], "Valid sequence");
    }
    #[test]
    fn context_manager_repoint_relation() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut context = Context::new(&slab);

        // 2 -> 1, 4 -> 3
        // Then:
        // 2 -> 4
        
        let head1 = context.add_test_subject(1, None, &slab        );
        let head2 = context.add_test_subject(2, Some(head1), &slab );
        let head3 = context.add_test_subject(3, None,        &slab );
        let head4 = context.add_test_subject(4, Some(head3), &slab );

        // Repoint Subject 2 slot 0 to subject 4
        let head2_b = slab.new_memo_basic(Some(2), head2, MemoBody::Relation(RelationSlotSubjectHead::single(0,4,head4) )).to_head();
        context.apply_head(4, &head2_b, &slab);

        let mut iter = context.subject_head_iter();
        assert!(iter.get_subject_ids() == [1,4,3,2], "Valid sequence");
    }
    #[test]
    fn context_manager_remove() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut context = Context::new(&slab);

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        context.apply_head(1, head1.project_all_edge_links(&slab), head1.clone());

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        context.apply_head(2, head2.project_all_edge_links(&slab), head2.clone());

        //Subject 3 slot 0 is pointing to Subject 2
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 2, head2.clone()) }).to_head();
        context.apply_head(3, head3.project_all_edge_links(&slab), head3.clone());


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        context.remove_head(2);
        
        let mut iter = context.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(3, iter.next().expect("iter result 3 should be present").subject_id);
        assert_eq!(1, iter.next().expect("iter result 1 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    #[test]
    fn context_manager_add_remove_cycle() {
        let net = Network::create_new_system();
        let slab = Slab::new(&net);
        let mut context = Context::new(&slab);

        // Subject 1 is pointing to nooobody
        let head1 = slab.new_memo_basic_noparent(Some(1), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        context.apply_head(1, head1.project_all_edge_links(&slab), head1.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        context.remove_head(1);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 1);

        // Subject 2 slot 0 is pointing to Subject 1
        let head2 = slab.new_memo_basic_noparent(Some(2), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 1, head1.clone()) }).to_head();
        context.apply_head(2, head2.project_all_edge_links(&slab), head2.clone());

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        context.remove_head(2);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        //Subject 3 slot 0 is pointing to nobody
        let head3 = slab.new_memo_basic_noparent(Some(3), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::empty() }).to_head();
        context.apply_head(3, head3.project_all_edge_links(&slab), head3.clone());

        assert_eq!(manager.subject_count(), 1);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 1);
        context.remove_head(3);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        // Subject 4 slot 0 is pointing to Subject 3
        let head4 = slab.new_memo_basic_noparent(Some(4), MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSlotSubjectHead::single(0, 3, head3.clone()) }).to_head();
        context.apply_head(4, head4.project_all_edge_links(&slab), head4);

        assert_eq!(manager.subject_count(), 2);
        assert_eq!(manager.subject_head_count(), 1);
        assert_eq!(manager.vacancies(), 0);
        context.remove_head(4);
        assert_eq!(manager.subject_count(), 0);
        assert_eq!(manager.subject_head_count(), 0);
        assert_eq!(manager.vacancies(), 2);

        let mut iter = context.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert!(iter.next().is_none(), "iter should have ended");
    }

    #[test]
    fn context_manager_contention() {

        use std::thread;
        use std::sync::{Arc,Mutex};

        let net = Network::create_new_system();
        let slab = Slab::new(&net);

        let interloper = Arc::new(Mutex::new(1));

        let mut manager = ContextManager::new_pathological(Box::new(|caller|{
            if caller == "pre_increment".to_string() {
                interloper.lock().unwrap();
            }
        }));


        let head1 = context.add_test_subject(1, None,        &slab);    // Subject 1 is pointing to nooobody

        let lock = interloper.lock().unwrap();
        let t1 = thread::spawn(|| {
            // should block at the first pre_increment
            let head2 = context.add_test_subject(2, Some(head1), &slab);    // Subject 2 slot 0 is pointing to Subject 1
            let head3 = context.add_test_subject(3, Some(head2), &slab);    // Subject 3 slot 0 is pointing to Subject 2
        });

        context.remove_head(1);
        drop(lock);

        t1.join();

        assert_eq!(manager.contains_subject(1),      true  );
        assert_eq!(manager.contains_subject_head(1), false );
        assert_eq!(manager.contains_subject_head(2), true  );
        assert_eq!(manager.contains_subject_head(3), true  );


        // 2[0] -> 1
        // 3[0] -> 2
        // Subject 1 should have indirect_references = 2

        
        let mut iter = context.subject_head_iter();
        // for subject_head in iter {
        //     println!("{} is {}", subject_head.subject_id, subject_head.indirect_references );
        // }
        assert_eq!(2, iter.next().expect("iter result 2 should be present").subject_id);
        assert_eq!(3, iter.next().expect("iter result 1 should be present").subject_id);
        assert!(iter.next().is_none(), "iter should have ended");
    }
    
}