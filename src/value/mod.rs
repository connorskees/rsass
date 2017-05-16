use colors::{name_to_rgb, rgb_to_name};
use error::Error;
use formalargs::{CallArgs, call_args};
use functions::get_builtin_function;
use nom::multispace;
use num_rational::Rational;
use num_traits::{One, Signed, Zero};
use operator::Operator;
use parseutil::{is_name_char, name, opt_spacelike, spacelike2};
use std::fmt;
use std::str::{FromStr, from_utf8};
use unit::{Unit, unit};
use variablescope::Scope;

/// A sass value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    /// A call has a name and an argument (which may be multi).
    Call(String, CallArgs),
    /// Sometimes an actual division, sometimes "a/b".
    /// In the later case, the booleans tell if there should be whitespace
    /// before / after the slash.
    Div(Box<Value>, Box<Value>, bool, bool),
    Literal(String, Quotes),
    List(Vec<Value>, ListSeparator),
    /// A Numeric value is a rational value with a Unit (which may be
    /// Unit::None) and a flag which is true for calculated values and
    /// false for literal values.
    Numeric(Rational, Unit, bool),
    /// "(a/b) and a/b differs semantically.  Parens means the value
    /// should be evaluated numerically if possible, without parens /
    /// is not allways division.
    Paren(Box<Value>),
    Variable(String),
    /// Both a numerical and original string representation,
    /// since case and length should be preserved (#AbC vs #aabbcc).
    Color(Rational, Rational, Rational, Rational, Option<String>),
    Null,
    True,
    False,
    /// A binary operation, two operands and an operator.
    BinOp(Box<Value>, Operator, Box<Value>),
    UnaryOp(Operator, Box<Value>),
    Interpolation(Box<Value>),
}

/// The difference between a comma-separated and a
/// whitespace-separated list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListSeparator {
    Comma,
    Space,
}

impl Value {
    pub fn scalar(v: isize) -> Self {
        Value::Numeric(Rational::from_integer(v), Unit::None, false)
    }
    pub fn bool(v: bool) -> Self {
        if v { Value::True } else { Value::False }
    }
    pub fn black() -> Self {
        let z = Rational::zero();
        Value::Color(z, z, z, Rational::one(), Some("black".into()))
    }
    pub fn rgba(r: Rational, g: Rational, b: Rational, a: Rational) -> Self {
        fn cap(n: Rational, ff: &Rational) -> Rational {
            if n > *ff {
                *ff
            } else if n.is_negative() {
                Rational::zero()
            } else {
                n
            }
        }
        let ff = Rational::new(255, 1);
        let one = Rational::one();
        Value::Color(cap(r, &ff), cap(g, &ff), cap(b, &ff), cap(a, &one), None)
    }

    pub fn type_name(&self) -> &'static str {
        match *self {
            Value::Color(..) => "color",
            Value::Literal(..) => "string",
            Value::Numeric(..) => "number",
            Value::List(..) => "list",
            Value::Null => "null",
            _ => "unknown",
        }
    }

    pub fn is_calculated(&self) -> bool {
        match *self {
            Value::Numeric(_, _, calculated) => calculated,
            Value::Color(_, _, _, _, None) => true,
            _ => false,
        }
    }

    /// All values other than `False` and `Null` should be considered true.
    pub fn is_true(&self) -> bool {
        match *self {
            Value::False | Value::Null => false,
            _ => true,
        }
    }

    pub fn is_null(&self) -> bool {
        match *self {
            Value::Null => true,
            Value::List(ref list, _) => list.iter().all(|v| v.is_null()),
            _ => false,
        }
    }

    pub fn integer_value(&self) -> Result<isize, Error> {
        match self {
            &Value::Numeric(ref num, _, _) if num.is_integer() => {
                Ok(num.to_integer())
            }
            v => Err(Error::bad_value("integer", v)),
        }
    }

    pub fn evaluate(&self, scope: &Scope) -> Value {
        self.do_evaluate(scope, false)
    }
    pub fn do_evaluate(&self, scope: &Scope, arithmetic: bool) -> Value {
        match *self {
            Value::Literal(ref v, ref q) => {
                Value::Literal(v.clone(), q.clone())
            }
            Value::Paren(ref v) => v.do_evaluate(scope, true),
            Value::Color(_, _, _, _, _) => self.clone(),
            Value::Variable(ref name) => {
                let v = scope.get(name);
                v.do_evaluate(scope, true)
            }
            Value::List(ref v, ref s) => {
                Value::List(v.iter()
                                .map(|v| v.do_evaluate(scope, false))
                                .collect::<Vec<_>>(),
                            s.clone())
            }
            Value::Call(ref name, ref args) => {
                match scope.call_function(name, args) {
                    Some(value) => value,
                    None => {
                        if let Some(function) = get_builtin_function(name) {
                            match function.call(scope, args) {
                                Ok(v) => v,
                                Err(e) => {
                                    panic!("Error in function {}: {:?}",
                                           name, e)
                                }
                            }
                        } else {
                            Value::Call(name.clone(), args.xyzzy(scope))
                        }
                    }
                }
            }
            Value::Div(ref a, ref b, ref space1, ref space2) => {
                let (a, b) = {
                    let aa = a.do_evaluate(scope, arithmetic);
                    let b =
                        b.do_evaluate(scope, arithmetic || a.is_calculated());
                    if !arithmetic && b.is_calculated() && !a.is_calculated() {
                        (a.do_evaluate(scope, true), b)
                    } else {
                        (aa, b)
                    }
                };
                if arithmetic || a.is_calculated() || b.is_calculated() {
                    match (&a, &b) {
                        (&Value::Color(ref r, ref g, ref b, ref a, _),
                         &Value::Numeric(ref n, Unit::None, _)) => {
                            Value::rgba(r / n, g / n, b / n, *a)
                        }
                        (&Value::Numeric(ref av, ref au, _),
                         &Value::Numeric(ref bv, ref bu, _)) => {
                            if bv.is_zero() {
                                Value::Div(Box::new(a.clone()),
                                           Box::new(b.clone()),
                                           *space1,
                                           *space2)
                            } else if bu == &Unit::None {
                                Value::Numeric(av / bv, au.clone(), true)
                            } else if au == bu {
                                Value::Numeric(av / bv, Unit::None, true)
                            } else {
                                Value::Div(Box::new(a.clone()),
                                           Box::new(b.clone()),
                                           false,
                                           false)
                            }
                        }
                        (a, b) => {
                            Value::Div(Box::new(a.clone()),
                                       Box::new(b.clone()),
                                       false,
                                       false)
                        }
                    }
                } else {
                    Value::Div(Box::new(a), Box::new(b), *space1, *space2)
                }
            }
            Value::Numeric(ref v, ref u, ref is_calculated) => {
                Value::Numeric(*v, u.clone(), arithmetic || *is_calculated)
            }
            Value::Null => Value::Null,
            Value::True => Value::True,
            Value::False => Value::False,
            Value::BinOp(ref a, ref op, ref b) => {
                op.eval(a.do_evaluate(scope, true), b.do_evaluate(scope, true))
            }
            Value::UnaryOp(ref op, ref v) => {
                Value::UnaryOp(op.clone(), Box::new(v.do_evaluate(scope, true)))
            }
            Value::Interpolation(ref v) => {
                match without_quotes(v.do_evaluate(scope, true)) {
                    Value::Null => Value::Null,
                    Value::Literal(s, _) => Value::Literal(s, Quotes::None),
                    v => Value::Literal(format!("{}", v), Quotes::None),
                }
            }
        }
    }
}

fn without_quotes(v: Value) -> Value {
    match v {
        Value::Literal(s, _) => Value::Literal(s, Quotes::None),
        Value::List(list, s) => {
            Value::List(list.into_iter().map(without_quotes).collect(), s)
        }
        v => v,
    }
}


/// A literal value can be double-quoted, single-quoted or not quoted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Quotes {
    Double,
    Single,
    None,
}

impl fmt::Display for Value {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &Value::Literal(ref s, ref q) => {
                match *q {
                    Quotes::Double => {
                        write!(out,
                               "\"{}\"",
                               s.chars()
                                   .flat_map(|c| match c {
                                                 '#' => vec!['\\', '#'],
                                                 '\\' => vec!['\\', '\\'],
                                                 '"' => vec!['\\', '"'],
                                                 c => vec![c],
                                             })
                                   .collect::<String>())
                    }
                    Quotes::Single => {
                        write!(out,
                               "'{}'",
                               s.chars()
                                   .flat_map(|c| match c {
                                                 '#' => vec!['\\', '#'],
                                                 '\\' => vec!['\\', '\\'],
                                                 '\'' => vec!['\\', '\''],
                                                 c => vec![c],
                                             })
                                   .collect::<String>())
                    }
                    Quotes::None => write!(out, "{}", s),
                }
            }
            &Value::Numeric(ref v, ref u, ref _is_calculated) => {
                let short = out.alternate();
                write!(out, "{}{}", rational2str(v, short), u)
            }
            &Value::Color(ref r, ref g, ref b, ref a, ref s) => {
                let r = r.round().to_integer() as u8;
                let g = g.round().to_integer() as u8;
                let b = b.round().to_integer() as u8;
                if let Some(ref s) = *s {
                    write!(out, "{}", s)
                } else if a >= &Rational::from_integer(1) {
                    if out.alternate() {
                        // E.g. #ff00cc can be written #f0c in css.
                        // 0xff / 17 = 0xf (since 17 = 0x11).
                        let hex = if r % 17 == 0 && g % 17 == 0 &&
                                     b % 17 == 0 {
                            format!("#{:x}{:x}{:x}",
                                    r / 17,
                                    g / 17,
                                    b / 17)
                        } else {
                            format!("#{:02x}{:02x}{:02x}", r, g, b)
                        };
                        match rgb_to_name(r, g, b) {
                            Some(name) if name.len() <= hex.len() => {
                                write!(out, "{}", name)
                            }
                            _ => write!(out, "{}", hex),
                        }
                    } else if let Some(name) = rgb_to_name(r, g, b) {
                        write!(out, "{}", name)
                    } else {
                        write!(out, "#{:02x}{:02x}{:02x}", r, g, b)
                    }
                } else if a.is_zero() && r.is_zero() && g.is_zero() &&
                          b.is_zero() {
                    write!(out, "transparent")
                } else if out.alternate() {
                    write!(out,
                           "rgba({},{},{},{})",
                           r,
                           g,
                           b,
                           rational2str(a, false))
                } else {
                    write!(out,
                           "rgba({}, {}, {}, {})",
                           r,
                           g,
                           b,
                           rational2str(a, false))
                }
            }
            &Value::List(ref v, ref sep) => {
                let t = v.iter()
                    .filter(|v| !v.is_null())
                    .map(|v| if out.alternate() {
                             format!("{:#}", v)
                         } else {
                             format!("{}", v)
                         })
                    .collect::<Vec<_>>()
                    .join(match *sep {
                              ListSeparator::Comma => {
                                  if out.alternate() { "," } else { ", " }
                              }
                              ListSeparator::Space => " ",
                          });
                write!(out, "{}", t)
            }
            &Value::Div(ref a, ref b, s1, s2) => {
                a.fmt(out)?;
                if s1 {
                    out.write_str(" ")?;
                }
                out.write_str("/")?;
                if s2 {
                    out.write_str(" ")?;
                }
                b.fmt(out)
            }
            &Value::Call(ref name, ref arg) => write!(out, "{}({})", name, arg),
            &Value::BinOp(ref a, Operator::Plus, ref b) => {
                // The plus operator is also a concat operator
                a.fmt(out)?;
                b.fmt(out)
            }
            &Value::BinOp(ref a, ref op, ref b) => {
                a.fmt(out)?;
                op.fmt(out)?;
                b.fmt(out)
            }
            &Value::Paren(ref v) => {
                out.write_str("(")?;
                v.fmt(out)?;
                out.write_str(")")
            }
            &Value::True => write!(out, "true"),
            &Value::False => write!(out, "false"),
            &Value::UnaryOp(ref op, ref v) => {
                op.fmt(out)?;
                v.fmt(out)
            }
            &Value::Variable(ref name) => {
                // Output as source in case it was not evaluated.
                write!(out, "${}", name)
            }
            &Value::Interpolation(ref value) => {
                // Output as source in case it was not evaluated.
                out.write_str("#{")?;
                value.fmt(out)?;
                out.write_str("}")
            }
            &Value::Null => Ok(()),
        }
    }
}

use std::cmp::Ordering;
impl PartialOrd for Value {
    fn partial_cmp(&self, b: &Value) -> Option<Ordering> {
        match (self, b) {
            (&Value::Numeric(ref a, _, _), &Value::Numeric(ref b, _, _)) => {
                a.partial_cmp(b)
            }
            _ => None,
        }
    }
}

fn rational2str(r: &Rational, skipzero: bool) -> String {
    if r.is_integer() {
        format!("{}", r.numer())
    } else {
        let prec = Rational::from_integer(100000);
        let v = (r * prec).round() / prec;
        let mut result = format!("{}", *v.numer() as f64 / *v.denom() as f64);
        if skipzero && result.starts_with("0.") {
            result.remove(0);
        }
        result
    }
}

named!(pub value_expression<&[u8], Value>,
       do_parse!(
           result: separated_nonempty_list!(
               do_parse!(tag!(",") >> opt_spacelike >> ()),
               space_list) >>
           trail: many0!(do_parse!(opt_spacelike >> tag!(",") >>
                                   opt_spacelike >>
                                   ())) >>
           (if result.len() == 1 && trail.is_empty() {
               result.into_iter().next().unwrap()
           } else {
               Value::List(result, ListSeparator::Comma)
           })));

named!(pub space_list<&[u8], Value>,
       do_parse!(first: single_expression >>
                 list: fold_many0!(
                     alt_complete!(
                         preceded!(multispace, single_expression) |
                         unquoted_literal |
                         variable),
                     vec![first],
                     |mut list: Vec<Value>, item| { list.push(item); list }) >>
                 (if list.len() == 1 {
                     list.into_iter().next().unwrap()
                 } else {
                     Value::List(list, ListSeparator::Space)
                 })));

named!(pub single_expression<Value>,
       do_parse!(a: logic_expression >>
                 r: fold_many0!(
                     do_parse!(opt!(multispace) >>
                               op: alt_complete!(
                                   value!(Operator::And,
                                          preceded!(tag!("and"),
                                                    spacelike2)) |
                                   value!(Operator::Or,
                                          preceded!(tag!("or"),
                                                    spacelike2))) >>
                               opt!(multispace) >>
                               b: single_expression >>
                               (op, b)),
                     a,
                     |a, (op, b)| Value::BinOp(Box::new(a), op, Box::new(b))) >>
                 (r)));

named!(pub logic_expression<Value>,
       do_parse!(a: sum_expression >>
                 r: fold_many0!(
                     do_parse!(opt!(multispace) >>
                               op: alt_complete!(
                                   value!(Operator::Equal, tag!("==")) |
                                   value!(Operator::NotEqual, tag!("!=")) |
                                   value!(Operator::GreaterE, tag!(">=")) |
                                   value!(Operator::Greater, tag!(">")) |
                                   value!(Operator::LesserE, tag!("<=")) |
                                   value!(Operator::Lesser, tag!("<"))) >>
                               opt!(multispace) >>
                               b: sum_expression >>
                               (op, b)),
                     a,
                     |a, (op, b)| Value::BinOp(Box::new(a), op, Box::new(b))) >>
                 (r)));

named!(pub sum_expression<Value>,
       do_parse!(a: term_value >>
                 r: fold_many0!(
                     alt_complete!(
                         do_parse!(op: alt_complete!(
                                       value!(Operator::Plus, tag!("+")) |
                                       value!(Operator::Minus, tag!("-"))) >>
                                   b: term_value >>
                                   (op, b)) |
                         do_parse!(spacelike2 >>
                                   op: alt_complete!(
                                       value!(Operator::Plus, tag!("+")) |
                                       value!(Operator::Minus, tag!("-"))) >>
                                   spacelike2 >>
                                   b: term_value >>
                                   (op, b))),
                     a,
                     |a, (op, b)| Value::BinOp(Box::new(a), op, Box::new(b))) >>
                 (r)));

named!(term_value<Value>,
       do_parse!(a: single_value >>
                 r: fold_many0!(
                     do_parse!(s1: opt!(multispace) >>
                               op: alt_complete!(tag!("*") | tag!("/")) >>
                               s2: opt!(multispace) >>
                               b: single_value >>
                               (s1.is_some(), op, s2.is_some(), b)),
                     a,
                     |a, (s1, op, s2, b)| {
                         if op == b"*" {
                             Value::BinOp(Box::new(a),
                                          Operator::Multiply,
                                          Box::new(b))
                         } else {
                             Value::Div(Box::new(a), Box::new(b), s1, s2)
                         }
                     }) >>
                 (r)));

named!(pub single_value<&[u8], Value>,
       alt_complete!(
           value!(Value::True, tag!("true")) |
           value!(Value::False, tag!("false")) |
           do_parse!(sign: opt!(alt!(tag!("-") | tag!("+"))) >>
                     r: is_a!("0123456789") >>
                     d: opt!(preceded!(tag!("."), is_a!("0123456789"))) >>
                     u: opt!(unit) >>
                     (Value::Numeric(
                         {
                             let d = Rational::from_str(
                                 from_utf8(r).unwrap()).unwrap() +
                                 d.map(decimals_to_rational)
                                 .unwrap_or_else(Rational::zero);
                             if sign == Some(b"-") { -d } else { d }
                         }
                         , u.unwrap_or(Unit::None), false))) |
           do_parse!(sign: opt!(alt!(tag!("-") | tag!("+"))) >>
                     tag!(".") >>
                     d: is_a!("0123456789") >>
                     u: opt!(unit) >>
                     (Value::Numeric(
                         {
                             let d = decimals_to_rational(d);
                             if sign == Some(b"-") { -d } else { d }
                         },
                         u.unwrap_or(Unit::None),
                         false))) |
           variable |
           do_parse!(tag!("#") >> r: hexchar2 >> g: hexchar2 >> b: hexchar2 >>
                     (Value::Color(from_hex(r),
                                   from_hex(g),
                                   from_hex(b),
                                   Rational::from_integer(1),
                                   Some(format!("#{}{}{}",
                                                from_utf8(r).unwrap(),
                                                from_utf8(g).unwrap(),
                                                from_utf8(b).unwrap()))))) |
           do_parse!(tag!("#") >> r: hexchar >> g: hexchar >> b: hexchar >>
                     (Value::Color(from_hex(r) * Rational::new(17, 1),
                                   from_hex(g) * Rational::new(17, 1),
                                   from_hex(b) * Rational::new(17, 1),
                                   Rational::from_integer(1),
                                   Some(format!("#{}{}{}",
                                                from_utf8(r).unwrap(),
                                                from_utf8(g).unwrap(),
                                                from_utf8(b).unwrap()))))) |
           function_call |
           unquoted_literal |
           map!(preceded!(tag!("-"), single_value),
                |s| Value::UnaryOp(Operator::Minus, Box::new(s))) |
           map!(preceded!(tag!("+"), single_value),
                |s| Value::UnaryOp(Operator::Plus, Box::new(s))) |
           map!(tag!("\"\""),
                |_| Value::Literal("".into(), Quotes::Double)) |
           quoted_string |
           map!(tag!("''"),
                |_| Value::Literal("".into(), Quotes::Single)) |
           singlequoted_string |
           map!(delimited!(preceded!(tag!("("), opt_spacelike),
                           opt!(value_expression),
                           terminated!(opt_spacelike, tag!(")"))),
                |val: Option<Value>| match val {
                    Some(v) => Value::Paren(Box::new(v)),
                    None => Value::List(vec![], ListSeparator::Space),
                })));

named!(variable<Value>,
       do_parse!(tag!("$") >>  name: name >> (Value::Variable(name))));

named!(interpolation<Value>,
       map!(delimited!(tag!("#{"), value_expression, tag!("}")),
            |v| Value::Interpolation(Box::new(v))));

named!(unquoted_literal<Value>,
       do_parse!(t: alt!(interpolation | unquoted_literal_part) >>
                 first: expr_res!(ok_as_literal(t)) >>
                 all: fold_many0!(
                     alt!(interpolation | function_call |
                          unquoted_literal_part |
                          map!(preceded!(tag!("//"),
                                         take_while1!(is_ext_str_char)),
                               |v| Value::Literal(
                                   format!("//{}", from_utf8(v).unwrap()),
                                   Quotes::None))),
                     first,
                     |a, b| {
                         Value::BinOp(Box::new(a), Operator::Plus, Box::new(b))
                     }) >>
                 (all)));

fn ok_as_literal(s: Value) -> Result<Value, bool> {
    if s != Value::Literal("-".into(), Quotes::None) {
        Ok(s)
    } else {
        Err(false)
    }
}

named!(function_call<Value>,
       do_parse!(name: name >> args: call_args >>
                 (Value::Call(name, args))));

fn is_ext_str_char(c: u8) -> bool {
    is_name_char(c) || c == b'*' || c == b'+' || c == b',' ||
    c == b'.' || c == b'/' || c == b':' || c == b'=' ||
    c == b'?' || c == b'|'
}

named!(unquoted_literal_part<Value>,
       map!(is_not!("+*/=;,$(){{}}! \n\t'\"#"), |val| {
           let val = from_utf8(val).unwrap().to_string();
           if val == "null" {
               Value::Null
           } else if let Some((r, g, b)) = name_to_rgb(&val) {
               Value::Color(r, g, b, Rational::from_integer(1), Some(val))
           } else {
               Value::Literal(val, Quotes::None)
           }
       }));

// a quoted string may contain interpolations
named!(quoted_string<Value>,
       do_parse!(tag!("\"") >>
                 first: simple_dqs_part >>
                 all: fold_many0!(
                     alt!(interpolation | nonempty_dqs_part),
                     first,
                     |a, b| {
                         Value::BinOp(Box::new(a), Operator::Plus, Box::new(b))
                     }) >>
                 tag!("\"") >> (all)));

named!(simple_dqs_part<Value>,
       map!(escaped!(is_not!("\\\"#"), '\\', take!(1)),
            |s| Value::Literal(unescape(from_utf8(s).unwrap()),
                               Quotes::Double)));
named!(nonempty_dqs_part<Value>,
       map!(verify!(escaped!(is_not!("\\\"#"), '\\', take!(1)),
                    |s: &[u8]| !s.is_empty()),
            |s| Value::Literal(unescape(from_utf8(s).unwrap()),
                               Quotes::Double)));

// a quoted string may contain interpolations
named!(singlequoted_string<Value>,
       do_parse!(tag!("'") >>
                 first: simple_sqs_part >>
                 all: fold_many0!(
                     alt!(interpolation | nonempty_sqs_part),
                     first,
                     |a, b| {
                         Value::BinOp(Box::new(a), Operator::Plus, Box::new(b))
                     }) >>
                 tag!("'") >> (all)));

named!(simple_sqs_part<Value>,
       map!(escaped!(is_not!("\\'#"), '\\', take!(1)),
            |s| Value::Literal(unescape(from_utf8(s).unwrap()),
                               Quotes::Single)));
named!(nonempty_sqs_part<Value>,
       map!(verify!(escaped!(is_not!("\\'#"), '\\', take!(1)),
                    |s: &[u8]| !s.is_empty()),
            |s| Value::Literal(unescape(from_utf8(s).unwrap()),
                               Quotes::Single)));

fn decimals_to_rational(d: &[u8]) -> Rational {
    Rational::new(from_utf8(d).unwrap().parse().unwrap(),
                  10_isize.pow(d.len() as u32))
}

named!(hexchar, recognize!(one_of!("0123456789ABCDEFabcdef")));

named!(hexchar2,
       recognize!(do_parse!(one_of!("0123456789ABCDEFabcdef") >>
                            one_of!("0123456789ABCDEFabcdef") >> ())));

fn from_hex(v: &[u8]) -> Rational {
    Rational::from_integer(u8::from_str_radix(from_utf8(v).unwrap(), 16)
                               .unwrap() as isize)
}

fn unescape(s: &str) -> String {
    let mut i = s.chars();
    let mut result = String::new();
    while let Some(c) = i.next() {
        result.push(match c {
                        '\\' => {
                            match i.next() {
                                Some(c) => c,
                                None => '\\',
                            }
                        }
                        c => c,
                    });
    }
    result
}

#[cfg(test)]
mod test;