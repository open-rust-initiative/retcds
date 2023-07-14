
use thiserror::Error;
#[derive(Debug, Error, PartialEq)]
pub enum Error{
    #[error("ioutil: short read")]
    ErrShortRead,

    #[error("ioutil: expect EOF")]
    ErrExpectEOF,

    #[error("send error")]
    ErrSend,

    #[error("snap: snapshot file doesn't exist")]
    ErrNoDBSnapshot,

    #[error("snap: no available snapshot")]
    ErrNoSnapshot,

    #[error("snap: empty snapshot")]
    ErrEmptySnapshot,

    #[error("snap: crc mismatch")]
    ErrCRCMismatch


}
