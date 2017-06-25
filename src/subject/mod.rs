mod core;
mod handle;

// use core;
// use handle;

struct Subject;

impl Subject {
    pub fn new ( context: &Context, vals: HashMap<String, String>, is_index: bool ) -> Result<SubjectHandle,String> {
        let slab = &context.slab;
        let subject_id = slab.generate_subject_id();
        //println!("# Subject({}).new()",subject_id);

        let memoref = slab.new_memo_basic_noparent(
                Some(subject_id),
                MemoBody::FullyMaterialized {v: vals, r: RelationSlotSubjectHead(HashMap::new()) }
            );
        let head = memoref.to_head();

        let core = core::SubjectCore(subject_id, head);

        context.subscribe_subject( &subject );

        // HACK HACK HACK - this should not be a flag on the subject, but something in the payload I think
        if !is_index {
            // NOTE: important that we do this after the subject.shared.lock is released
            context.insert_into_root_index( subject_id, &subject );
        }
        Ok(subject)
    }
    pub fn reconstitute (context: ContextHandle, head: MemoRefHead) -> SubjectCore {
        //println!("Subject.reconstitute({:?})", head);
        let context = contextref.get_context();

        let subject_id = head.first_subject_id().unwrap();

        let core = SubjectCore(Arc::new(SubjectInner{
            id: subject_id,
            head: RwLock::new(head),
            contextref: contextref
        }));

        let arc = Arc::new(core)
        context.subscribe_subject( &arc );

        subject
    }
    pub fn new_blank ( context: &ContextHandle ) -> Result<Subject,String> {
        Self::new( context, HashMap::new(), false )
    }
    pub fn new_kv ( context: &ContextHandle, key: &str, value: &str ) -> Result<Subject,String> {
        let mut vals = HashMap::new();
        vals.insert(key.to_string(), value.to_string());

        Self::new( context, vals, false )
    }
}