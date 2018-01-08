use core;

#[derive(PartialEq, Debug)]
pub enum RetrieveError {
    NotFound,
    NotFoundByDeadline,
    AccessDenied,
    InvalidMemoRefHead,
    IndexNotInitialized,
    SlabError,
    MemoLineageError,
}

#[derive(PartialEq, Debug)]
pub enum ObserveError{
    Unknown
}

impl core::convert::From<()> for ObserveError {
    fn from(error: ()) -> Self {
        ObserveError::Unknown
    }
}