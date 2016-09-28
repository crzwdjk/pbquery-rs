use super::parser::{Path,RawFilter,RawItem};
use ::query::{PBExpr,PBFilter, PBItem};
use ::descriptors::{MessageDescriptor,FieldDescriptor,Label};

use std::collections::HashSet;
extern crate libloading;

type TypecheckResult<T> = Result<T, &'static str>;

fn tc_path(item: RawItem, context: &FieldDescriptor) ->
    TypecheckResult<(PBItem, ::descriptors::Type)>
{
    let r = match item {
        RawItem::Path(p) => {
            let md = context.get_message_descriptor();
            let fieldmessage = try!(md.ok_or("Not a message"));
            let result = try!(typecheck(*p, fieldmessage));
            let expr_type = result.expr_type;
            (PBItem::Path(result), expr_type)
        },
        RawItem::AtItem => (PBItem::At, context.fieldtype),
        _ => return Err("Expected path, found atom")
    };
    Ok(r)
}

fn tc_atom(atom: RawItem) -> TypecheckResult<PBItem> {
    Ok(match atom {
        RawItem::IntItem(i) => PBItem::Int(i),
        RawItem::FloatItem(f) => PBItem::Float(f),
        RawItem::StrItem(s) => PBItem::Str(s),
        _ => return Err("Expected atom, found path"),
    })
}

fn tc_eq(lhs: RawItem, rhs: RawItem, invert: bool,
                context: &FieldDescriptor)
                -> TypecheckResult<PBFilter> {
    if lhs.is_atom() && rhs.is_atom() {
        return constant_fold(lhs, rhs, invert);
    }
    let (rawpath, rawatom) = if lhs.is_path() {(lhs, rhs)} else {(rhs, lhs)};

    let atom = try!(tc_atom(rawatom).
                    or(Err("comparing two paths is not supported")));

    let (path, pathtype) = try!(tc_path(rawpath, context));
    match atom {
        PBItem::Int(_) if pathtype.is_inty() => true,
        PBItem::Float(_) if pathtype.is_floaty() => true,
        PBItem::Str(_) if pathtype.is_stringy() => true,
        _ => return Err("type mismatch"),
    };
       
    Ok(PBFilter::EqFilter { atom: atom,
                            path: path,
                            invert: invert })
}

fn constant_fold(lhs: RawItem, rhs: RawItem, invert: bool)
                 -> Result<PBFilter, &'static str> {
    assert!(lhs.is_atom() && rhs.is_atom());
    let val = match (lhs, rhs) {
        (RawItem::IntItem(i1), RawItem::IntItem(i2)) => i1 == i2,
        (RawItem::FloatItem(f1), RawItem::FloatItem(f2)) => f1 == f2,
        (RawItem::StrItem(s1), RawItem::StrItem(s2)) => s1 == s2,
        _ => false
    };
    if val && !invert || !val && invert {
        Ok(PBFilter::TrueFilter)
    } else {
        Err("Constant folding produced false")
    }
}

fn tc_int_list(list: Vec<RawItem>) -> TypecheckResult<HashSet<i32>> {
    if !list.iter().all(|i| i.is_int()) {
        return Err("Expected a list of literal ints");
    }
    Ok(list.into_iter().map(
        |item| if let RawItem::IntItem(i) = item { i }
        else { unreachable!() }
    ).collect())
}

fn tc_str_list(list: Vec<RawItem>) -> TypecheckResult<HashSet<String>> {
    if !list.iter().all(|i| i.is_str()) {
        return Err("Expected a list of literal strings");
    }
    Ok(list.into_iter().map(
        |item| if let RawItem::StrItem(s) = item { s }
        else { unreachable!() }
    ).collect())
}

fn tc_in(rawitem: RawItem, list: Vec<RawItem>, context: &FieldDescriptor)
                -> TypecheckResult<PBFilter> {
    let (item, itype) = try!(tc_path(rawitem, context));
    if itype.is_inty() {
        let l = try!(tc_int_list(list));
        Ok(PBFilter::InIntFilter(item, l))
    } else if itype.is_stringy() {
        let l = try!(tc_str_list(list));
        Ok(PBFilter::InStrFilter(item, l))
    } else {
        Err("Operator 'in' only supports ints or strings")
    }
}
    
fn tc_filter(rawfilter: RawFilter, context: &FieldDescriptor)
                    -> Result<PBFilter, &'static str> {
    match rawfilter {
        RawFilter::TrueFilter => Ok(PBFilter::TrueFilter),
        RawFilter::EqFilter(lhs, rhs, inv) =>
            tc_eq(lhs, rhs, inv, context),
        /* Check that RHS is a string, LHS is a string message */
        RawFilter::RxFilter(lhs, rhs, inv) => unimplemented!(),
        RawFilter::InFilter(item, list) => tc_in(item, list, context),
        RawFilter::IdxFilter(i) =>
            if context.label == Label::REPEATED && i >= 0 {
                Ok(PBFilter::IdxFilter(i as u32))
            } else {
                Err("bad index")
            },
    }
}

pub fn typecheck(rawpath: Path, rootmessage: &MessageDescriptor)
             -> Result<PBExpr, &'static str> {
    let mut message = rootmessage;
    let mut paths = vec!();
    let mut filters = vec!();
    let mut types = vec!();
    let len = rawpath.len();
    for part in rawpath {
        let field = message.get_field_by_name(part.path);
        let f = try!(field.ok_or("No such field"));

        filters.push(try!(tc_filter(part.filter, f)));

        paths.push(f.id);
        types.push(f.fieldtype);
        match f.get_message_descriptor() {
            Some(m) => message = m,
            None => break,
        }
    };

    if paths.len() != len {
        return Err("Could not match some fields");
    }
    let t = try!(types.last().ok_or("Empty path"));
    Ok(PBExpr { path: paths, filters: filters, expr_type: *t})
}
