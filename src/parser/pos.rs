use super::Span;
use std::str::from_utf8;

/// Position data for a parse error.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct SourcePos {
    pub line: String,
    pub line_no: usize,
    pub line_pos: usize,
    pub file: SourceName,
}

impl SourcePos {
    pub fn magic_pos(span: Span) -> Self {
        use std::slice;

        let self_bytes = span.fragment();
        let self_ptr = self_bytes.as_ptr();
        let offset = span.get_column() - 1;
        let the_line = unsafe {
            assert!(
                offset <= isize::max_value() as usize,
                "offset is too big"
            );
            let orig_input_ptr = self_ptr.offset(-(offset as isize));
            slice::from_raw_parts(
                orig_input_ptr,
                offset + span.fragment().len(),
            )
        };
        let the_line = the_line
            .split(|c| *c == b'\n')
            .next()
            .and_then(|s| from_utf8(s).ok())
            .unwrap_or("<<failed to display line>>");

        SourcePos {
            line: the_line.to_string(),
            line_no: span.location_line() as usize,
            line_pos: span.get_utf8_column(),
            file: span.extra.clone(),
        }
    }
}

/// The name of a scss source file.
///
/// This also contains the information if this was the root stylesheet
/// or where it was imported from.
#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct SourceName {
    name: String,
    imported: Option<Box<SourcePos>>,
}

impl SourceName {
    pub fn root<T: ToString>(name: T) -> Self {
        SourceName {
            name: name.to_string(),
            imported: None,
        }
    }
    pub fn imported<T: ToString>(name: T, from: SourcePos) -> Self {
        SourceName {
            name: name.to_string(),
            imported: Some(Box::new(from)),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn imported_from(&self) -> Option<&SourcePos> {
        self.imported.as_ref().map(|b| b.as_ref())
    }
}
