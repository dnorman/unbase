
#[derive(PartialEq, Debug)]
pub enum Error{
    RetrieveError(RetrieveError),
    WriteError(WriteError),
    TransmitError(TransmitError),
    ObserveError(ObserveError),
    StorageOpDeclined(StorageOpDeclined),
    LocalSlab(LocalSlabError),
}

#[derive(PartialEq, Debug)]
pub enum RetrieveError {
    NotFound,
    NotFoundLocally,
    NotFoundByDeadline,
    InsufficientPeering,
    AccessDenied,
    InvalidMemoRefHead,
    IndexNotInitialized,
    SlabError,
    MemoLineageError
}

#[derive(PartialEq, Debug)]
pub enum TransmitError{
    SlabPresenceNotFound,
    InvalidTransmitter
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
    InsufficientPeering,
    InvalidAddress,
}

#[derive(PartialEq, Debug)]
pub enum LocalSlabError{
    Unreachable
}