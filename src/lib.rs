pub mod pbiter;
pub mod query;
mod descriptors;
mod compiler;

pub use descriptors::MessageDescriptor;
use pbiter::PBMessage;
use std::ptr::null;
use std::ffi::CStr;
use std::slice;
extern crate libc;

pub fn compile(expr: &str, rootmessage: &MessageDescriptor)
           -> Result<PBExpr, &'static str> {
    let raw = try!(compiler::parser::parse(expr));
    compiler::typecheck::typecheck(raw, rootmessage)
}
pub use query::{query, query_stream};
use query::PBExpr;

#[no_mangle]
pub unsafe fn pbquery_compile(cexpr: *const libc::c_char,
                              prootmessage: *const MessageDescriptor)
                              -> *const PBExpr {
    let expr = match CStr::from_ptr(cexpr).to_str() {
        Ok(s) => s,
        Err(_) => return null(),
    };
    let rootmessage = match prootmessage.as_ref() {
        Some(r) => r,
        None => return null(),
    };

    match compile(expr, rootmessage) {
        Ok(r) => &r,
        Err(_) => null(),
    }
}

#[repr(C)]
pub struct C_PBMessage {
    buf: *const u8,
    len: usize,
    tag: u32,
    wiretype: pbiter::WireType,
}
pub type CCallback = extern fn(msg: *const C_PBMessage,
                               cbdata: *const libc::c_void) -> bool;
#[no_mangle]
pub unsafe fn pbquery_run(cexpr: *const PBExpr, buf: *const u8, len: usize,
                          callback: CCallback, cbdata: *mut libc::c_void)
                          -> () {
    let expr = match cexpr.as_ref() {
        None => return,
        Some(r) => r,
    };
    let msg = slice::from_raw_parts(buf, len);
    let mut cb = |message: PBMessage| callback(&C_PBMessage {
        buf: message.contents.as_ptr(),
        len: message.contents.len(),
        tag: message.tag,
        wiretype: message.wiretype },
                                cbdata);
    query::query(msg, expr, &mut cb);
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
    }
}
