use super::core;
use context::core::ContextCore;

#[derive(Clone)]
struct SubjectHandle {
    pub core:    Arc<SubjectCore>,
    pub context: Arc<ContextCore>
}

impl SubjectHandle{
    pub fn get_value ( &self, key: &str ) -> Option<String> {
        self.core.get_value(&self.context, key)
    }
    pub fn get_relation ( &self, key: RelationSlotId ) -> Result<Subject, RetrieveError> {
        self.core.get_relation(&self.context, key)
    }
    pub fn set_value (&self, key: &str, value: &str) -> bool {
        self.core.set_value(&self.context, key, value)
    }
    pub fn set_relation (&self, key: RelationSlotId, relation: &Self) {
        self.core.set_relation(&self.context, key, relation)
    }
    pub fn get_all_memo_ids ( &self ) -> Vec<MemoId> {
        self.core.get_all_memo_ids(&self.context)
    }
}


impl fmt::Debug for SubjectHandle {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Subject")
            .field("subject_id", &self.core.id)
            .field("head", &self.core.head)
            .finish()
    }
}