use std::io::prelude::*;


fn read_varint(buf: &[u8]) -> (usize, usize) {
    let mut acc = 0 as usize;
    let mut cnt = 0 as usize;
    for b in buf {
        acc += ((b & 0x7f) as usize) << (cnt * 7);
        cnt += 1;
        if b & 0x80 == 0 { return (acc, cnt) }
    }
    panic!("Ill formed varint");
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub enum WireType { VARINT = 0, FIXED64 = 1, LENGTH_PREFIXED = 2, FIXED32 = 5 }
fn wire_type(tagbits: u8) -> WireType {
    match tagbits {
        0 => WireType::VARINT,
        1 => WireType::FIXED64,
        2 => WireType::LENGTH_PREFIXED,
        5 => WireType::FIXED32,
        _ => panic!("Bad wiretype {}", tagbits),
    }
}

#[derive(Clone, Copy)]
pub struct PBMessage<'a> {
    pub contents: &'a [u8],
    pub tag: u32,
    pub wiretype: WireType,
    pub bytes: &'a [u8],
}

impl<'a> PBMessage<'a> {
    pub fn as_int(&self) -> i32 {
        match self.wiretype {
            WireType::VARINT => read_varint(self.contents).0 as i32,
            _ => unimplemented!()
        }
    }
    pub fn as_float(&self) -> f64 {
        match self.wiretype {
            WireType::FIXED32 => {
                assert!(self.contents.len() == 4);
                let p = self.contents.as_ptr() as *const f32;
                unsafe { *p as f64 }
            },
            WireType::FIXED64 => {
                assert!(self.contents.len() == 8);
                let p = self.contents.as_ptr() as *const f64;
                unsafe { *p }
            },
            _ => panic!("Not a float"),
        }
    }
    pub fn as_str(&self) -> &str {
        ::std::str::from_utf8(self.contents).unwrap()
    }
}
    
pub struct PBIter<'a> {
    buf: &'a [u8],
}

impl<'a> PBIter<'a> {
    pub fn new(buf: &[u8]) -> PBIter {
        PBIter { buf: buf }
    }
    pub fn len(&self) -> usize { self.buf.len() }
}

    
impl<'a> Iterator for PBIter<'a> {
    type Item = PBMessage<'a>;

    fn next(&mut self) -> Option<PBMessage<'a>> {
        if self.buf.is_empty() { return None }
        let (rawtag, taglen) = read_varint(self.buf);
        let wiretype = wire_type((rawtag & 0x7) as u8);
        let rest = self.buf.split_at(taglen).1;
        if rest.is_empty() { return None }
        let (len, start) = match wiretype {
            WireType::FIXED64 => (8, 0),
            WireType::FIXED32 => (4, 0),
            WireType::LENGTH_PREFIXED => read_varint(rest),
            WireType::VARINT => (read_varint(rest).1, 0),
        };
        if rest.len() < start + len { return None }
        let splits = rest[start..].split_at(len);
        let origbuf = self.buf;
        self.buf = splits.1;
        let msgsize = taglen + start + len;

//        println!("Msg wiretype {:?}, tag {:?}, contentlen {}, totallen {}",
//                 wiretype, rawtag >> 3, splits.0.len(), msgsize);
        return Some(PBMessage { contents: splits.0,
                                tag: (rawtag >> 3) as u32,
                                wiretype: wiretype,
                                bytes: &origbuf[0..msgsize],
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
