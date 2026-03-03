use super::*;

pub struct StrLines<'a>(Vec<&'a str>);

impl<'a> From<&'a str> for StrLines<'a> {
    fn from(s: &'a str) -> Self {
        Self(s.lines().collect())
    }
}
impl<'a> StrLines<'a> {
    pub fn map_pos(&self, pos: SourcePos) -> SourcePos {
        let line_idx = pos.line.saturating_sub(1);
        let line_src = self.0.get(line_idx).copied().unwrap_or("");
        let col = line_src
            .chars()
            .take(pos.column)
            .map(|c| c.len_utf16())
            .sum();
        SourcePos {
            line: pos.line,
            column: col,
        }
    }

    pub fn map_loc(&self, loc: SourceLoc) -> SourceLoc {
        SourceLoc {
            begin: self.map_pos(loc.begin),
            end: self.map_pos(loc.end),
        }
    }

    pub fn map_range(&self, range: IRSourceRange) -> SourceLoc {
        let (begin_pos, end_pos) = range;
        self.map_loc(SourceLoc {
            begin: SourcePos {
                line: begin_pos.line,
                column: begin_pos.column_nchars,
            },
            end: SourcePos {
                line: end_pos.line,
                column: end_pos.column_nchars,
            },
        })
    }
}
