#[derive(Debug)]
pub enum Error {
    RetrieveError(RetrieveError),
    WriteError(WriteError),
    TransmitError(TransmitError),
    ObserveError(ObserveError),
    StorageOpDeclined(StorageOpDeclined),
    //    LocalSlab(LocalSlabError),
    Buffer(BufferError),
    Serde(serde_json::Error),
    TransmitterNotFound,
    SlabOffline,
    SlabNotFound,
    ChannelNotFound,
    BadAddress,
    AddressNotFound,
    StorageError(StorageError),
}

#[derive(Debug)]
pub enum StorageError {
    SledError(sled::Error),
}

#[derive(Debug)]
pub enum BufferError {
    BincodeDecodeFailed(bincode::Error),
}

#[derive(PartialEq, Debug)]
pub enum RetrieveError {
    NotFound,
    NotFoundByDeadline,
    AccessDenied,
    InvalidHead(InvalidHead),
    IndexNotInitialized,
    SlabError,
    MemoLineageError,
    WriteError(Box<WriteError>),
}

#[derive(PartialEq, Debug)]
pub enum InvalidHead {
    MissingEntityId,
    Empty,
}

#[derive(PartialEq, Debug)]
pub enum TransmitError {
    SlabPresenceNotFound,
    InvalidTransmitter,
}

#[derive(PartialEq, Debug)]
pub enum WriteError {
    Unknown,
    RetrieveError(Box<RetrieveError>),
    // This is silly. TODO - break this cycle and remove the Box
    BadTarget,
}

#[derive(PartialEq, Debug)]
pub enum ObserveError {
    Unknown,
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
impl core::convert::From<bincode::Error> for Error {
    fn from(error: bincode::Error) -> Self {
        Error::Buffer(BufferError::BincodeDecodeFailed(error))
    }
}

#[derive(PartialEq, Debug)]
pub enum StorageOpDeclined {
    InsufficientPeering,
}

#[derive(PartialEq, Debug)]
pub enum PeeringError {
    InsufficientPeering,
}
