use css;
use functions::get_builtin_function;
use num_rational::Rational;
use num_traits::{One, Signed, Zero};
use sass::CallArgs;
use std::fmt;
use value::{ListSeparator, Operator, Quotes, Unit, rgb_to_name};
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
    /// Unit::None) and flags.
    ///
    /// The first flag is true for values with an explicit + sign.
    ///
    /// The second flag is true for calculated values and false for
    /// literal values.
    Numeric(Rational, Unit, bool, bool),
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

impl Value {
    pub fn scalar(v: isize) -> Self {
        Value::Numeric(Rational::from_integer(v), Unit::None, false, false)
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
            Value::True | Value::False => "bool",
            Value::Null => "null",
            _ => "unknown",
        }
    }

    pub fn is_calculated(&self) -> bool {
        match *self {
            Value::Numeric(_, _, _, calculated) => calculated,
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

    pub fn evaluate(&self, scope: &Scope) -> css::Value {
        self.do_evaluate(scope, false)
    }
    pub fn do_evaluate(&self, scope: &Scope, arithmetic: bool) -> css::Value {
        match *self {
            Value::Literal(ref v, ref q) => {
                css::Value::Literal(v.clone(), q.clone())
            }
            Value::Paren(ref v) => v.do_evaluate(scope, true),
            Value::Color(r, g, b, a, ref s) => {
                css::Value::Color(r, g, b, a, s.clone())
            }
            Value::Variable(ref name) => scope.get(name).into_calculated(),
            Value::List(ref v, ref s) => {
                css::Value::List(v.iter()
                                     .map(|v| v.do_evaluate(scope, false))
                                     .collect::<Vec<_>>(),
                                 s.clone())
            }
            Value::Call(ref name, ref args) => {
                let args = args.evaluate(scope);
                match scope.call_function(name, &args) {
                    Some(value) => value,
                    None => {
                        if let Some(function) = get_builtin_function(name) {
                            match function.call(scope, &args) {
                                Ok(v) => v,
                                Err(e) => {
                                    panic!("Error in function {}: {:?}",
                                           name, e)
                                }
                            }
                        } else {
                            css::Value::Call(name.clone(), args)
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
                        (&css::Value::Color(ref r, ref g, ref b, ref a, _),
                         &css::Value::Numeric(ref n, Unit::None, ..)) => {
                            css::Value::rgba(r / n, g / n, b / n, *a)
                        }
                        (&css::Value::Numeric(ref av, ref au, ..),
                         &css::Value::Numeric(ref bv, ref bu, ..)) => {
                            if bv.is_zero() {
                                css::Value::Div(Box::new(a.clone()),
                                                Box::new(b.clone()),
                                                *space1,
                                                *space2)
                            } else if bu == &Unit::None {
                                css::Value::Numeric(av / bv,
                                                    au.clone(),
                                                    false,
                                                    true)
                            } else if au == bu {
                                css::Value::Numeric(av / bv,
                                                    Unit::None,
                                                    false,
                                                    true)
                            } else {
                                css::Value::Div(Box::new(a.clone()),
                                                Box::new(b.clone()),
                                                false,
                                                false)
                            }
                        }
                        (a, b) => {
                            css::Value::Div(Box::new(a.clone()),
                                            Box::new(b.clone()),
                                            false,
                                            false)
                        }
                    }
                } else {
                    css::Value::Div(Box::new(a), Box::new(b), *space1, *space2)
                }
            }
            Value::Numeric(ref v, ref u, ref sign, ref calc) => {
                css::Value::Numeric(*v, u.clone(), *sign, arithmetic || *calc)
            }
            Value::Null => css::Value::Null,
            Value::True => css::Value::True,
            Value::False => css::Value::False,
            Value::BinOp(ref a, ref op, ref b) => {
                op.eval(a.do_evaluate(scope, true), b.do_evaluate(scope, true))
            }
            Value::UnaryOp(ref op, ref v) => {
                let value = v.do_evaluate(scope, true);
                match (op.clone(), value) {
                    (Operator::Not, css::Value::Numeric(v, ..)) => {
                        css::Value::bool(v.is_zero())
                    }
                    (Operator::Not, css::Value::True) => css::Value::False,
                    (Operator::Not, css::Value::False) => css::Value::True,
                    (Operator::Minus, css::Value::Numeric(v, u, ..)) => {
                        css::Value::Numeric(-v, u, false, true)
                    }
                    (Operator::Plus, css::Value::Numeric(v, u, ..)) => {
                        css::Value::Numeric(v, u, true, true)
                    }
                    (op, v) => css::Value::UnaryOp(op, Box::new(v)),
                }
            }
            Value::Interpolation(ref v) => {
                match v.do_evaluate(scope, true).unquote() {
                    css::Value::Null => css::Value::Null,
                    css::Value::Literal(s, _) => {
                        css::Value::Literal(s, Quotes::None)
                    }
                    v => css::Value::Literal(format!("{}", v), Quotes::None),
                }
            }
        }
    }

    pub fn unquote(self) -> Value {
        match self {
            Value::Literal(s, _) => Value::Literal(s, Quotes::None),
            Value::List(list, s) => {
                Value::List(list.into_iter().map(|v| v.unquote()).collect(), s)
            }
            v => v,
        }
    }
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
            &Value::Numeric(ref v, ref u, ref with_sign, _) => {
                let short = out.alternate();
                write!(out, "{}{}", rational2str(v, *with_sign, short), u)
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
                           rational2str(a, false, false))
                } else {
                    write!(out,
                           "rgba({}, {}, {}, {})",
                           r,
                           g,
                           b,
                           rational2str(a, false, false))
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
            (&Value::Numeric(ref a, ..), &Value::Numeric(ref b, ..)) => {
                a.partial_cmp(b)
            }
            _ => None,
        }
    }
}

fn rational2str(r: &Rational, with_sign: bool, skipzero: bool) -> String {
    if r.is_integer() {
        if with_sign {
            format!("{:+}", r.numer())
        } else {
            format!("{}", r.numer())
        }
    } else {
        let prec = Rational::from_integer(100000);
        let v = (r * prec).round() / prec;
        let v = *v.numer() as f64 / *v.denom() as f64;
        let mut result = if with_sign {
            format!("{:+}", v)
        } else {
            format!("{}", v)
        };
        if skipzero && result.starts_with("0.") {
            result.remove(0);
        }
        result
    }
}
