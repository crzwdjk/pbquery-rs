use pbiter::*;
use ::descriptors::{Type};
use std::collections::HashSet;

#[derive(Debug)]
pub enum PBItem {
    Int(i32),
    Float(f64),
    Str(String),
    At,
    Path(PBExpr),
}

#[derive(Debug)]
pub enum PBFilter {
    EqFilter { atom: PBItem, path: PBItem, invert: bool },
    InStrFilter(PBItem, HashSet<String>),
    InIntFilter(PBItem, HashSet<i32>),
    IdxFilter(u32),
    TrueFilter,
}

fn eval_path<'a>(path: &PBItem, msg: &PBMessage<'a>) -> Option<PBMessage<'a>> {
    let mut ret = None;
    match path {
        &PBItem::At => Some(*msg),
        &PBItem::Path(ref p) => {
            query(msg.contents, p, &mut |m| { ret = Some(m); false});
            ret
        }
        _ => panic!("Not a path!")
    }
}

            
impl PBFilter {
    fn eval(&self, msg: &PBMessage) -> bool {
        match self {
            &PBFilter::TrueFilter => { true },
            &PBFilter::EqFilter { ref atom, ref path, invert } => {
                let submsg = match eval_path(path, msg) {
                    None => return false,
                    Some(m) => m,
                };
                let v = match atom {
                    &PBItem::Int(i) => submsg.as_int() == i,
                    &PBItem::Float(f) => submsg.as_float() == f,
                    &PBItem::Str(ref s) => submsg.as_str() == s,
                    _ => unimplemented!()
                };
                if invert { !v } else { v }
            },
            _ => unimplemented!()
        }
    }
}

#[derive(Debug)]
pub struct PBExpr {
    pub path: Vec<u32>,
    pub filters: Vec<PBFilter>,
    pub expr_type: Type,
}

pub struct Subexpr<'a> {
    path: &'a [u32],
    filters: &'a [PBFilter],
}

fn query_helper<'a, 'b, F>(msg: &'a [u8], expr: Subexpr<'b>, callback: &mut F)
                           -> usize
    where F : FnMut(PBMessage<'a>) -> bool
{
    let targettag = expr.path[0];
    let ref filter = expr.filters[0];
    let mut bytes = 0;
    {
        // need this block to bound the lifetime of the closure that
        // mutably borrows bytes
        let matches = PBIter::new(msg).inspect(|m| bytes += m.bytes.len())
                                      .filter(|p| p.tag == targettag)
                                      .filter(|p| filter.eval(p));
        for m in matches {
            if expr.path.len() == 1 {
                if !callback(m) { break; }
            } else {
                let subpath = Subexpr { path: &expr.path[1..],
                                        filters: &expr.filters[1..], };
                query_helper(m.contents, subpath, callback);
            }
        }
    }
    bytes
}

pub fn query<'a, F>(msg: &'a [u8], expr: &PBExpr, callback: &mut F) -> usize
    where F : FnMut(PBMessage<'a>) -> bool
{
    assert!(expr.path.len() > 0);
    let subpath = Subexpr { path: &expr.path[..],
                            filters: &expr.filters[..] };
    query_helper(msg, subpath, callback)
}


use std::io::BufRead;
use std::io::Read;

pub fn query_stream<F>(stream: &mut BufRead,
                    expr: &PBExpr,
                    mut callback: F) -> ()
    where F : FnMut(PBMessage) -> bool,
{
    loop {
        let l = {
            let buf = stream.fill_buf().unwrap();
            println!("buflen: {}", buf.len());
            if buf.len() == 0 { return; }            
            query(buf, expr, &mut callback)
        };
        if l == 0 { return; }
        println!("consuming {}", l);
        stream.consume(l);
    }
}
