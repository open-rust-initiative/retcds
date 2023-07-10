use std::io;
use std::io::{BufRead, Read};
use futures::{AsyncRead, SinkExt, TryFutureExt};
use thiserror::Error;
use crate::etcdserver::api::rafthttp::error::Error;

pub type Result<T> = std::result::Result<T, Error>;

pub struct ExactReaderCloser {
    rc : Box<dyn Read>,
    br : i64,
    total_bytes : i64,
}

impl ExactReaderCloser{
    pub fn new(rc : Box<dyn Read>, total_bytes : i64) -> Self {
        ExactReaderCloser {
            rc : rc,
            br : Default::default(),
            total_bytes : total_bytes,
        }
    }
    pub fn read(&mut self, p: &mut [u8]) -> Result<usize> {
        let n = self.rc.read(p).unwrap();
        self.br += n as i64;
        if self.br > self.total_bytes {
           return  Err(Error::ErrExpectEOF)
        };
        if self.br < self.total_bytes && n == 0 {
            return Err(Error::ErrShortRead)
        };
        Ok(n)
    }

    // pub async fn close(&mut self) -> Result<()>{
    //     let x = self.rc.as_mut().;
    //     if x.is_err() {
    //         return Err(Error::ErrShortRead)
    //     };
    //
    //
    //     if self.br < self.total_bytes {
    //         return Err(Error::ErrShortRead)
    //     };
    //
    // }

}

#[test]
fn test_exact_reader_closer_ExpectEOF() {
    use std::io::Cursor;
    // use crate::ExactReaderCloser;
    let x = "hello".as_bytes();
    let mut r = ExactReaderCloser::new(Box::new(x), 3);
    let mut buf = [0; 3];
    let n = r.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(buf, ['h' as u8, 'e' as u8, 'l' as u8]);
    let n = r.read(&mut buf).unwrap_err();
    // println!("{}", n);
    assert_eq!(n, Error::ErrExpectEOF);
    // assert_eq!(buf, ['h' as u8, 'e' as u8, 'l' as u8]);
    // assert_eq!(err, Error::ErrExpectEOF);
}

#[test]
fn test_exact_reader_closer_ShortRead() {
    use std::io::Cursor;
    // use crate::ExactReaderCloser;

    let mut r = ExactReaderCloser::new(Box::new("hello".as_bytes()), 6);
    let mut buf = [0; 3];
    let n = r.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(buf, ['h' as u8, 'e' as u8, 'l' as u8]);
    let n = r.read(&mut buf).unwrap();
    assert_eq!(n, 2);
    assert_eq!(buf, ['l' as u8, 'o' as u8, 'l' as u8]);
    let n = r.read(&mut buf).unwrap_err();
    assert_eq!(n, Error::ErrShortRead);
}