
use std::io::{BufReader, Read};

// #[derive(Clone)]
pub struct LimitedBufferReader {
    r: Box<dyn Read>,
    n: usize,
}



impl LimitedBufferReader{
    pub fn new(r: Box<dyn Read>, n: usize) -> Self {
        LimitedBufferReader {
            r: r,
            n: n,
        }
    }
    fn read(&mut self, p: &mut [u8]) -> std::io::Result<usize> {
        let mut np = p;
        if np.len() > self.n {
            np = &mut np[..self.n];
        };
        return self.r.read(np);
    }
}

#[test]
fn test_limited_buffer_reader() {
    let mut r = LimitedBufferReader::new(Box::new(BufReader::new("hello".as_bytes())), 3);
    let mut buf = [0; 3];
    let n = r.read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(buf, ['h' as u8, 'e' as u8, 'l' as u8]);
    let n = r.read(&mut buf).unwrap();
    assert_eq!(n, 2);
    assert_eq!(buf, ['l' as u8, 'o' as u8, 'l' as u8]);
    let n = r.read(&mut buf).unwrap();
    assert_eq!(n, 0);
    assert_eq!(buf, ['l' as u8, 'o' as u8, 'l' as u8]);
}