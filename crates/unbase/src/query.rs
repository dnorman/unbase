use context::Context;
use subject::{SubjectId,SubjectField};

pub struct Query{
    parts: Vec<QueryPart>,
    context: Context
}
pub struct QueryPart {
    fields: Vec<SubjectField>,
    criteria: QueryPartCriteria
}
pub enum QueryPartCriteria {
    BySubjectId: { id: Vec<SubjectId> }
    ByFieldComparison
}

impl Query {
    pub fn new (context: &Context) -> Query {
        Query {
            parts:   Vec::new(),
            context: context
        }
    }

    pub fn add_part {

    }
}
