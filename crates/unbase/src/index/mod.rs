use subject::Subject;

mod fixed;
pub use self::fixed::IndexFixed;

trait Index{
    fn insert(&self, u64, Subject);
    fn get(&self, u64) -> Option<Subject>;
}
