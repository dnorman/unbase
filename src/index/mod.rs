use crate::subject::*;

mod fixed;
pub use self::fixed::IndexFixed;

trait Index{
    fn insert(&self, subject: Subject);
    fn get(&self, subject_id: SubjectId) -> Option<Subject>;
}
