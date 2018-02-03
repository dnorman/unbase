
#[derive(PartialEq, Debug)]
pub enum Error{
    RetrieveError(RetrieveError),
    WriteError(WriteError),
    TransmitError(TransmitError),
    ObserveError(ObserveError),
    StorageOpDeclined(StorageOpDeclined),
}

#[derive(PartialEq, Debug)]
pub enum RetrieveError {
    NotFound,
    NotFoundByDeadline,
    AccessDenied,
    InvalidMemoRefHead,
    IndexNotInitialized,
    SlabError,
    MemoLineageError
}

#[derive(PartialEq, Debug)]
pub enum TransmitError{
    SlabPresenceNotFound
}

#[derive(PartialEq, Debug)]
pub enum WriteError{

}

#[derive(PartialEq, Debug)]
pub enum ObserveError{
    Unknown
}

#[derive(PartialEq, Debug)]
pub enum StorageOpDeclined{
    InsufficientPeering
}