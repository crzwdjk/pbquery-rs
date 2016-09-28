pub type ParseError = &'static str;
pub type ParseResult<'a, T> = Result<(T, &'a str), ParseError>;
type Parser<'i, T> = Box<Fn(&'i str) -> ParseResult<'i, T> + 'i>;

fn is_id_start(ch: char) -> bool {
    ch.is_alphabetic() || ch == '_'
}

fn is_id_continue(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn ident<'a>(input: &'a str) -> ParseResult<'a, &'a str> {
    let head = input.chars().next();
    if head.is_none() || !is_id_start(head.unwrap()) {
        return Err("Expected identifier");
    }
    for (i, c) in input.char_indices() {
        if !is_id_continue(c) {
            return Ok(input.split_at(i));
        }
    }
    return Ok((input, ""));
}

#[derive(PartialEq)]
enum Num { Inty(i32), Floaty(f64) }

extern {
    fn strtod(s: *const u8, endptr: *mut *mut u8) -> f64;
    fn strtol(s: *const u8, endptr: *mut *mut u8) -> isize;
}
fn parsenum<'a>(input: &'a str) -> ParseResult<'a, Num> {
    let mut endf: *mut u8 = ::std::ptr::null_mut();
    let mut endi: *mut u8 = ::std::ptr::null_mut();

    let in_cstr = input.as_ptr(); // TODO: not very safe.
    let fval = unsafe { strtod(in_cstr, &mut endf as *mut *mut u8) };
    let ival = unsafe { strtol(in_cstr, &mut endi as *mut *mut u8) as i32 };
    if endi as usize == 0 { return Err("Not a number"); }

    let intbytes = endi as usize - in_cstr as usize;
    let floatbytes = endf as usize - in_cstr as usize;
    if floatbytes == 0 {
        Err("Not a number")
    } else if intbytes == floatbytes {
        Ok((Num::Inty(ival), &input[intbytes..]))
    } else {
        Ok((Num::Floaty(fval), &input[floatbytes..]))
    }
}

fn quoted_string<'a>(input: &'a str) -> ParseResult<'a, String> {
    let delim = input.chars().nth(0).unwrap();
    if delim != '\'' && delim != '"' { return Err("No quotes?"); }
    let mut result = String::new();
    let mut escape = false;
    for (i, c) in input[1..].char_indices() {
        if !escape {
            if c == delim { return Ok((result, &input[i + 2..])) }
            if c == '\\' { escape = true; continue }
        } else {
            escape = false;
        }
        result.push(c);
    }
    Err("No trailing delimiter?")
}

/////////////////////////////////
#[derive(Debug)]
pub enum RawItem<'a> {
    Path(Box<Path<'a>>),
    AtItem,
    IntItem(i32),
    FloatItem(f64),
    StrItem(String),
    ListItem(Vec<RawItem<'a>>)
}
impl<'a> RawItem<'a> {
    pub fn is_atom(&self) -> bool {
        match self {
            &RawItem::FloatItem(_) | &RawItem::IntItem(_)
                | &RawItem::StrItem(_) => true,
            _ => false,
        }
    }

    pub fn is_path(&self) -> bool {
        match self {
            &RawItem::AtItem | &RawItem::Path(_) => true,
            _ => false,
        }
    }
    pub fn is_int(&self) -> bool {
        if let &RawItem::IntItem(_) = self { true } else { false }
    }
    pub fn is_str(&self) -> bool {
        if let &RawItem::IntItem(_) = self { true } else { false }
    }
}

fn parse_list(input: &str) -> ParseResult<Vec<RawItem>> {
    let mut tail = input;
    let mut result = Vec::new();
    loop {
        tail = tail.trim_left();
        match parse_item(tail) {
            Ok((i, t)) => { result.push(i); tail = t }
            Err(_) => break,
        }
        tail = tail.trim_left();
        match tail.chars().nth(0) {
            Some(',') => tail = &tail[1..],
            _ => break,
        }
    }
    Ok((result, tail))
}

fn parse_item(input: &str) -> ParseResult<RawItem> {
    let first = try!(input.chars().nth(0).ok_or("End of input"));
    if first == '@' {
        Ok((RawItem::AtItem, &input[1..]))
    } else if first == '\'' || first == '"' {
        let (s, tail) = try!(quoted_string(input));
        Ok((RawItem::StrItem(s), tail))
    } else if first.is_numeric() || first == '-' || first == '+' {
        let (n, tail) = try!(parsenum(input));
        match n {
            Num::Inty(i) => Ok((RawItem::IntItem(i), tail)),
            Num::Floaty(f) => Ok((RawItem::FloatItem(f), tail)),
        }
    } else if first == '(' {
        let (l, tail) = try!(parse_list(&input[1..]));
        let tail = tail.trim_left();
        match tail.chars().nth(0) {
            Some('(') => Ok((RawItem::ListItem(l), &tail[1..])),
            _ => Err("Could not parse list")
        }
    } else {
        let (p, tail) = try!(parse_path(input));
        Ok((RawItem::Path(Box::new(p)), tail))
    }
}


enum Op { Eq, NotEq, Rx, NotRx, In }
fn parse_op(input: &str) -> ParseResult<Op> {
    let errmsg = "Expected operator, got end of input";
    let ch1 = try!(input.chars().nth(0).ok_or(errmsg));
    let ch2 = try!(input.chars().nth(1).ok_or(errmsg)); 
    match (ch1, ch2) {
        ('=', '=') => Ok((Op::Eq, &input[2..])),
        ('=', _) => Ok((Op::Eq, &input[1..])),
        ('~', _) => Ok((Op::Rx, &input[1..])),
        ('!', '=') => Ok((Op::NotEq, &input[2..])),
        ('!', '~') => Ok((Op::NotRx, &input[2..])),
        ('i', 'n') => Ok((Op::In, &input[2..])),
        _ => Err("Invalid operator")
    }
}

#[derive(Debug)]
pub enum RawFilter<'a> {
    TrueFilter,
    EqFilter(RawItem<'a>, RawItem<'a>, bool),
    RxFilter(RawItem<'a>, RawItem<'a>, bool),
    InFilter(RawItem<'a>, Vec<RawItem<'a>>),
    IdxFilter(i32),
}
fn parse_expr<'a>(input: &'a str) -> ParseResult<'a, RawFilter> {
    let tail = input.trim_left();
    let (left, tail) = try!(parse_item(tail));
    let tail = tail.trim_left();
    if let Ok((op, tail)) = parse_op(tail) {
        let tail = tail.trim_left();
        let (right, tail) = try!(parse_item(tail));
        let result = match op {
            Op::Eq => RawFilter::EqFilter(left, right, false),
            Op::NotEq => RawFilter::EqFilter(left, right, true),
            Op::Rx => RawFilter::RxFilter(left, right, false),
            Op::NotRx => RawFilter::RxFilter(left, right, true),
            Op::In => if let RawItem::ListItem(l) = right {
                RawFilter::InFilter(left, l)
            } else { return Err("right hand of 'in' must be a list") }
            //_ => return Err("Filter not implemented yet")
        };
        return Ok((result, tail))
    } else {
        if let RawItem::IntItem(i) = left {
            Ok((RawFilter::IdxFilter(i), tail))
        } else {
            Err("Could not parse filter")
        }
    }

}

#[derive(Debug)]
pub struct PathPart<'a> {
    pub path: &'a str,
    pub filter: RawFilter<'a>
}
pub type Path<'a> = Vec<PathPart<'a>>;

fn parse_path<'a>(input: &'a str) -> ParseResult<'a, Path<'a>> {
    let mut tail = input;
    let mut parts = Vec::new();
    while let Ok((id, t)) = ident(tail) {
        tail = t;
        let filter = if let Some('[') = tail.chars().nth(0) {
            let (f, t) = try!(parse_expr(&tail[1..]));
            if let Some(']') = t.chars().nth(0) {
                tail = &t[1..];
                f
            } else {
                return Err("couldn't find trailing ]");
            }
        } else {
            RawFilter::TrueFilter
        };
        parts.push(PathPart { path: id, filter: filter });
        if let Some('.') = tail.chars().nth(0) { tail = &tail[1..] } else { break }
    }
    Ok((parts, tail))
}

pub fn parse<'a>(input: &'a str) -> Result<Path<'a>, ParseError> {
    let (result, tail) = try!(parse_path(input));
    if tail.len() != 0 {
        println!("\nTrailing garbage: {}", tail);
        return Err("Trailing garbage after string");
    }
    return Ok(result);
}

#[cfg(test)]
mod tests {
    use super::parsenum;
    use super::{parse_item, parse_path, parse};
    use super::RawItem;
    
    #[test]    
    fn test_parsenum() {
        assert!(parsenum("goat").is_err());
        assert!(parsenum("-goat").is_err());
        assert!(parsenum("g6").is_err());
        assert!(parsenum("-").is_err());

        assert!(parsenum("42").unwrap().0 == super::Num::Inty(42));
        assert!(parsenum("-42").unwrap().0 == super::Num::Inty(-42));
        assert!(parsenum("+42").unwrap().0 == super::Num::Inty(42));
        assert!(parsenum("42.").unwrap().0 == super::Num::Floaty(42.0));
        assert!(parsenum("42.goat").unwrap().0 == super::Num::Floaty(42.0));
        assert!(parsenum("42.goat").unwrap().1 == "goat");
    }

    #[test]
    fn test_parseitem() {
        parse_item("42").unwrap();
        parse_item("-1").unwrap();
        assert!(parse_item("'foo'").unwrap().1 == "");
        let item = parse_item("'foo'").unwrap().0;
        assert!(if let RawItem::StrItem(s) = item { s == "foo" } else { false });
        let item = parse_item("'\\'\\\\\"'").unwrap().0;
        assert!(if let RawItem::StrItem(s) = item { s == "'\\\"" } else { false });
    }

    #[test]
    fn test_parsepath() {
        parse("foo").unwrap();
        parse("foo.bar").unwrap();
        parse("foo.bar[baz = 42].quux").unwrap();
        parse("foo.bar['goat' = baz].quux").unwrap();
        assert!(parse("[bar]").is_err());
        assert!(parse("bar[]").is_err());
        assert!(parse("bar[@]").is_err());
        assert!(parse("bar['foo']").is_err());
    }
}
