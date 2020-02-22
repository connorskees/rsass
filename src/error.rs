use crate::css::Value;
use crate::parser::{ParseError, SourcePos};
use std::convert::From;
use std::path::PathBuf;
use std::string::FromUtf8Error;
use std::{fmt, io};

/// Most functions in rsass that returns a Result uses this Error type.
#[derive(Debug)]
pub enum Error {
    Input(PathBuf, io::Error),
    IoError(io::Error),
    Encoding(FromUtf8Error),
    BadValue(String),
    BadArguments(String),
    ParseError { msg: String, pos: SourcePos },
    S(String),
    UndefinedVariable(String),
}

impl std::error::Error for Error {}

impl Error {
    pub fn bad_value(expected: &str, actual: &Value) -> Self {
        Error::BadValue(format!(
            "expected {}, got {} = {}",
            expected,
            actual.type_name(),
            actual.format(Default::default())
        ))
    }

    /// Wrong kind of argument to a sass function.
    /// `expected` is a string describing what the parameter should
    /// have been, `actual` is the argument.
    pub fn badarg(expected: &str, actual: &Value) -> Error {
        Error::BadArguments(format!(
            "expected {}, got {} = {}",
            expected,
            actual.type_name(),
            actual.format(Default::default())
        ))
    }

    /// Multiple-argument variant of `badarg`.
    pub fn badargs(expected: &[&str], actual: &[&Value]) -> Error {
        // TODO Better message!
        Error::BadArguments(format!(
            "expected {:?}, got {:?}",
            expected, actual
        ))
    }

    pub fn undefined_variable(name: &str) -> Self {
        Error::UndefinedVariable(name.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::S(ref s) => write!(out, "{}", s),
            Error::Input(ref p, ref e) => {
                write!(out, "Failed to read {:?}: {}", p, e)
            }
            Error::UndefinedVariable(ref name) => {
                write!(out, "Undefined variable: \"${}\"", name)
            }
            Error::ParseError { ref msg, ref pos } => {
                let line_no = pos.line_no.to_string();
                write!(
                    out,
                    "{msg}\
                     \n{0:lnw$} ,\
                     \n{ln} | {line}\
                     \n{0:lnw$} |{0:>lpos$}^\
                     \n{0:lnw$} '",
                    "",
                    line = pos.line,
                    msg = msg,
                    ln = line_no,
                    lnw = line_no.len(),
                    lpos = pos.line_pos,
                )?;
                let mut nextpos = Some(pos);
                while let Some(pos) = nextpos {
                    write!(
                        out,
                        "\n{0:lnw$} {file} {row}:{col}  {cause}",
                        "",
                        lnw = line_no.len(),
                        file = pos.file.name(),
                        row = pos.line_no,
                        col = pos.line_pos,
                        cause = if pos.file.imported_from().is_some() {
                            "import"
                        } else {
                            "root stylesheet"
                        },
                    )?;
                    nextpos = pos.file.imported_from();
                }
                Ok(())
            }
            // fallback
            ref x => write!(out, "{:?}", x),
        }
    }
}

impl<'a> From<ParseError<'a>> for Error {
    fn from(err: ParseError) -> Self {
        Error::ParseError {
            msg: format!("Parse error: {:?}", err.err),
            pos: SourcePos::magic_pos(err.span),
        }
    }
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::IoError(e)
    }
}

impl From<FromUtf8Error> for Error {
    fn from(e: FromUtf8Error) -> Self {
        Error::Encoding(e)
    }
}
