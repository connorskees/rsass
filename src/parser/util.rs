use super::Span;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::multispace1;
use nom::combinator::{map, not, opt, recognize, value};
use nom::multi::{fold_many0, fold_many1, many0};
use nom::sequence::{preceded, terminated};
use nom::IResult;

pub fn spacelike(input: Span) -> IResult<Span, ()> {
    fold_many1(alt((ignore_space, ignore_lcomment)), (), |(), ()| ())(input)
}

pub fn spacelike2(input: Span) -> IResult<Span, ()> {
    terminated(spacelike, ignore_comments)(input)
}

pub fn opt_spacelike(input: Span) -> IResult<Span, ()> {
    fold_many0(alt((ignore_space, ignore_lcomment)), (), |(), ()| ())(input)
}

pub fn ignore_comments(input: Span) -> IResult<Span, ()> {
    fold_many0(
        alt((ignore_space, ignore_lcomment, map(comment, |_| ()))),
        (),
        |(), ()| (),
    )(input)
}

pub fn comment(input: Span) -> IResult<Span, Span> {
    preceded(tag("/*"), comment2)(input)
}

pub fn comment2(input: Span) -> IResult<Span, Span> {
    terminated(
        recognize(many0(alt((
            value((), is_not("*")),
            preceded(tag("*"), not(tag("/"))),
        )))),
        tag("*/"),
    )(input)
}

pub fn ignore_space(input: Span) -> IResult<Span, ()> {
    map(multispace1, |_| ())(input)
}

fn ignore_lcomment(input: Span) -> IResult<Span, ()> {
    map(terminated(tag("//"), opt(is_not("\n"))), |_| ())(input)
}

#[cfg(test)]
mod test {
    use super::{comment, Span};

    #[test]
    fn comment_simple() {
        do_test(b"/* hello */\n", b" hello ", b"\n")
    }

    #[test]
    fn comment_with_stars() {
        do_test(b"/**** hello ****/\n", b"*** hello ***", b"\n")
    }

    #[test]
    fn comment_with_stars2() {
        do_test(
            b"/* / * / * / * hello * \\ * \\ * \\ */\n",
            b" / * / * / * hello * \\ * \\ * \\ ",
            b"\n",
        )
    }

    fn do_test(src: &[u8], content: &[u8], trail: &[u8]) {
        use crate::test_span;
        assert_eq!(
            comment(test_span!(src))
                .map(|(t, c)| (*t.fragment(), *c.fragment())),
            Ok((trail, content)),
        )
    }
}
