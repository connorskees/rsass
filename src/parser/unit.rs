use super::Span;
use crate::value::Unit;
use nom::IResult;
use nom::Slice;

pub fn unit(input: Span) -> IResult<Span, Unit> {
    for (n, u) in &[
        // Distance units, <length> type
        (&b"em"[..], Unit::Em),
        (b"ex", Unit::Ex),
        (b"ch", Unit::Ch),
        (b"rem", Unit::Rem),
        (b"vw", Unit::Vw),
        (b"vh", Unit::Vh),
        (b"vmin", Unit::Vmin),
        (b"vmax", Unit::Vmax),
        (b"cm", Unit::Cm),
        (b"mm", Unit::Mm),
        (b"q", Unit::Q),
        (b"in", Unit::In),
        (b"pt", Unit::Pt),
        (b"pc", Unit::Pc),
        (b"px", Unit::Px),
        // <angle> type
        (b"deg", Unit::Deg),
        (b"grad", Unit::Grad),
        (b"rad", Unit::Rad),
        (b"turn", Unit::Turn),
        // <time> type
        (b"s", Unit::S),
        (b"ms", Unit::Ms),
        // <frequency> type
        (b"Hz", Unit::Hz),
        (b"kHz", Unit::Khz),
        // <resolution>
        (b"dpi", Unit::Dpi),
        (b"dpcm", Unit::Dpcm),
        (b"dppx", Unit::Dppx),
        // Special units
        (b"fr", Unit::Fr),
        (b"%", Unit::Percent),
    ] {
        if input.fragment().starts_with(n) {
            return Ok((input.slice(n.len()..), u.clone()));
        }
    }
    Ok((input, Unit::None))
}
