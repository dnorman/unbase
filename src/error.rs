use core;
use serde_json;

#[derive(Debug)]
pub enum Error{
    RetrieveError(RetrieveError),
    WriteError(WriteError),
    //TransmitError(TransmitError),
    ObserveError(ObserveError),
    StorageOpDeclined(StorageOpDeclined),
    //LocalSlab(LocalSlabError),
    Buffer(BufferError),     Serde(serde_json::Error),
}

#[derive(Debug)]
pub enum BufferError{
    DecodeFailed
}

#[derive(PartialEq, Debug)]
pub enum RetrieveError {
    NotFound,
    NotFoundByDeadline,
    AccessDenied,
    InvalidMemoRefHead,
    IndexNotInitialized,
    SlabError,
    MemoLineageError,
    WriteError(Box<WriteError>),
}

#[derive(PartialEq, Debug)]
pub enum WriteError{
    RetrieveError(Box<RetrieveError>)
}

#[derive(PartialEq, Debug)]
pub enum ObserveError{
    Unknown
}

impl core::convert::From<()> for ObserveError {
    fn from(_error: ()) -> Self {
        ObserveError::Unknown
    }
}
impl core::convert::From<RetrieveError> for WriteError {
    fn from(error: RetrieveError) -> Self {
        WriteError::RetrieveError(Box::new(error))
    }
}
impl core::convert::From<WriteError> for RetrieveError {
    fn from(error: WriteError) -> Self {
        RetrieveError::WriteError(Box::new(error))
    }
}

#[derive(PartialEq, Debug)]
pub enum StorageOpDeclined{
    InsufficientPeering
}
