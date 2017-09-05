use super::*;
use subjecthandle::SubjectHandle;
use std::fmt;

/// User interface functions - Programmer API for `Context`
impl Context {
    /// Retrive a Subject from the root index by ID
    pub fn get_subject_by_id(&self, subject_id: SubjectId) -> Result<SubjectHandle, RetrieveError> {

        match *self.root_index.read().unwrap() {
            Some(ref index) => {
                Ok(
                    SubjectHandle{
                        subject: index.get(&self, subject_id)?,
                        context: self.clone()
                    }
                )
            }
            None => Err(RetrieveError::IndexNotInitialized),
        }
    }

    // Magically transport subject heads into another context in the same process.
    // This is a temporary hack for testing purposes until such time as proper context exchange is enabled
    // QUESTION: should context exchanges be happening constantly, but often ignored? or requested? Probably the former,
    //           sent based on an interval and/or compaction ( which would also likely be based on an interval and/or present context size)
    pub fn hack_send_context(&self, other: &Self) -> usize {
        self.compress();

        let from_slabref = self.slab.my_ref.clone_for_slab(&other.slab);

        let mut memoref_count = 0;

        for head in self.stash.iter() {
            memoref_count += head.len();
            other.apply_head(&head.clone_for_slab(&from_slabref, &other.slab, false));
        }

        memoref_count
    }
    pub fn get_resident_subject_head(&self, subject_id: SubjectId) -> Option<MemoRefHead> {
        if let Some(ref head) = self.stash.get_head(subject_id) {
            Some((*head).clone())
        } else {
            None
        }
    }
    pub fn get_resident_subject_head_memo_ids(&self, subject_id: SubjectId) -> Vec<MemoId> {
        if let Some(head) = self.get_resident_subject_head(subject_id) {
            head.memo_ids()
        } else {
            vec![]
        }
    }
    pub fn cmp(&self, other: &Self) -> bool {
        // stable way:
        &*(self.0) as *const _ != &*(other.0) as *const _

        // unstable way:
        // Arc::ptr_eq(&self.inner,&other.inner)
    }

    pub fn add_test_subject(&self, subject_id: SubjectId, maybe_relation: Option<SubjectId>, slab: &Slab) -> MemoRefHead {
        let relset = if let Some(subject_id) = maybe_relation {
            RelationSet::single(0, subject_id )
        }else{
            RelationSet::empty()
        };
        let memobody = MemoBody::FullyMaterialized { v: HashMap::new(), r: relset, e: EdgeSet::empty(), t: SubjectType::Record };
        let head = slab.new_memo_basic_noparent(Some(subject_id), memobody).to_head();

        self.apply_head(&head)
    }

    /// Attempt to compress the present query context.
    /// We do this by issuing Relation memos for any subject heads which reference other subject heads presently in the query context.
    /// Then we can remove the now-referenced subject heads, and repeat the process in a topological fashion, confident that these
    /// referenced subject heads will necessarily be included in subsequent projection as a result.
    pub fn compress(&self){
        unimplemented!()
        //self.manager.compress(&self.slab);
        // TODO2: Implement this, or remove it
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