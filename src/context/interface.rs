use super::*;
use subjecthandle::SubjectHandle;
use std::fmt;

/// User interface functions - Programmer API for `Context`
impl Context {
    /// Retrive a Subject from the root index by ID
    pub fn get_subject_by_id(&self, subject_id: SubjectId) -> Result<Option<SubjectHandle>, RetrieveError> {

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
    pub fn hack_send_context(&self, other: &Self) -> usize {
        self.compact().expect("compact");

        let from_slabref = self.slab.my_ref.clone_for_slab(&other.slab);

        let mut memoref_count = 0;

        for head in self.stash.iter() {
            memoref_count += head.len();
            println!("HACK SEND CONTEXT {} ({:?}) From {} to {}",  head.subject_id().unwrap(), head.memo_ids(), self.slab.id, other.slab.id );
            other.apply_head(&head.clone_for_slab(&from_slabref, &other.slab, false)).expect("apply head");
        }

        memoref_count
    }
    pub fn get_relevant_subject_head(&self, subject_id: SubjectId) -> Result<MemoRefHead, RetrieveError> {
        match subject_id {
            SubjectId{ stype: SubjectType::IndexNode,.. } => {
                Ok(self.stash.get_head(subject_id).clone())
            },
            SubjectId{ stype: SubjectType::Record, .. } => {
                // TODO: figure out a way to noop here in the case that the SubjectHead in question
                //       was pulled against a sufficiently identical context stash state.
                //       Perhaps stash edit increment? how can we get this to be really granular?

                match self.root_index()?.get_head(&self, subject_id.id)? {
                    Some(mrh) => Ok(mrh),
                    None      => Ok(MemoRefHead::Null)
                }
            }
        }
    }
    pub fn root_index (&self) -> Result<Arc<IndexFixed>,RetrieveError> {

        {
           let rg = self.root_index.read().unwrap();
           if let Some( ref arcindex ) = *rg {
               return Ok( arcindex.clone() )
           }
        };
        
        let seed = self.slab.get_root_index_seed();
        if seed.is_some() {
            let index = IndexFixed::new_from_memorefhead(&self, 5, seed);
            let arcindex = Arc::new(index);
            *self.root_index.write().unwrap() = Some(arcindex.clone());

            Ok(arcindex)
        }else{
            Err(RetrieveError::IndexNotInitialized)
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

        let memobody = MemoBody::FullyMaterialized { v: HashMap::new(), r: RelationSet::empty(), e: edgeset, t: subject_id.stype };
        let head = self.slab.new_memo_basic_noparent(Some(subject_id), memobody).to_head();

        self.apply_head(&head).expect("apply head")
    }

    /// Attempt to compress the present query context.
    /// We do this by issuing Relation memos for any subject heads which reference other subject heads presently in the query context.
    /// Then we can remove the now-referenced subject heads, and repeat the process in a topological fashion, confident that these
    /// referenced subject heads will necessarily be included in subsequent projection as a result.
    pub fn compact(&self) -> Result<(), WriteError>  {
        let before = self.stash.concise_contents();

        //TODO: implement topological MRH iterator for stash
        //      non-topological iteration will yield sub-optimal compaction

        // iterate all heads in the stash
        for parent_mrh in self.stash.iter() {
            let mut updated_edges = EdgeSet::empty();

            for edgelink in parent_mrh.project_occupied_edges(&self.slab)? {
                if let EdgeLink::Occupied{slot_id,head:edge_mrh} = edgelink {

                    if let Some(subject_id) = edge_mrh.subject_id(){
                        if let stash_mrh @ MemoRefHead::Subject{..} = self.stash.get_head(subject_id) {
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
                let head = self.slab.new_memo_basic(Some(subject_id), parent_mrh, memobody.clone()).to_head();
                
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