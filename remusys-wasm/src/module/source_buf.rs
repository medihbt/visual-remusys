use std::{
    fmt::{Debug, Display},
    ops::{Range, RangeFrom},
};

use smallvec::SmallVec;
use wasm_bindgen::JsError;

use crate::{MonacoSrcPos, SourcePosIndex, SourceRangeIndex, fmt_jserr};

type LineBuf = SmallVec<[u8; 32]>;

#[derive(Debug, Clone)]
pub struct SourceLine {
    buffer: LineBuf,
    is_ascii: bool,
}

impl SourceLine {
    fn from_linebuf(linebuf: &[u8]) -> Self {
        if cfg!(debug_assertions) {
            // In debug mode, we can afford to check the UTF-8 validity of the line buffer
            if let Err(e) = std::str::from_utf8(linebuf) {
                panic!("invalid UTF-8 in source line: {e}");
            }
            if linebuf.contains(&b'\n') {
                panic!("source line contains newline character");
            }
        }
        let is_ascii = linebuf.iter().all(|&b| b.is_ascii());
        Self {
            buffer: SmallVec::from_slice(linebuf),
            is_ascii,
        }
    }

    pub fn as_str(&self) -> &str {
        // # Safety
        //
        // The buffer must contain valid UTF-8 data.
        unsafe { std::str::from_utf8_unchecked(&self.buffer) }
    }

    pub fn byte_col_to_utf16(&self, byte_col: u32) -> Result<u32, JsError> {
        if byte_col > self.buffer.len() as u32 {
            let self_len = self.buffer.len();
            return fmt_jserr!(Err
                "invalid byte column: {byte_col} exceeds line length {self_len}\n\
                line content: {}",
                self.as_str()
            );
        }
        if byte_col == self.buffer.len() as u32 {
            return Ok(self.as_str().chars().map(|x| x.len_utf16() as u32).sum());
        }
        if self.is_ascii {
            // For ASCII lines, byte column and UTF-16 column are the same
            return Ok(byte_col);
        }
        let mut utf16_col = 0;
        let mut byte_index = 0;
        for ch in self.as_str().chars() {
            if byte_index >= byte_col as usize {
                return Ok(utf16_col);
            }
            utf16_col += ch.len_utf16() as u32;
            byte_index += ch.len_utf8();
        }
        if byte_index == byte_col as usize {
            Ok(utf16_col)
        } else {
            fmt_jserr!(Err "invalid byte column: {byte_col} is in the middle of a character")
        }
    }

    pub fn utf16_col_to_byte(&self, utf16_col: u32) -> Result<u32, JsError> {
        if self.is_ascii && utf16_col < self.buffer.len() as u32 {
            // For ASCII lines, UTF-16 column and byte column are the same
            return Ok(utf16_col);
        } else if utf16_col >= self.buffer.len() as u32 {
            return fmt_jserr!(Err "invalid UTF-16 column: {utf16_col} exceeds line length");
        }

        let mut current_utf16_col = 0;
        let mut byte_index = 0;
        for ch in self.as_str().chars() {
            if current_utf16_col >= utf16_col {
                return Ok(byte_index);
            }
            current_utf16_col += ch.len_utf16() as u32;
            byte_index += ch.len_utf8() as u32;
        }
        if current_utf16_col == utf16_col {
            Ok(byte_index)
        } else {
            fmt_jserr!(Err "invalid UTF-16 column: {utf16_col} is in the middle of a character")
        }
    }

    fn extend_with_str(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
        if self.is_ascii {
            self.is_ascii = s.is_ascii();
        }
    }
    fn replace_with_str(&mut self, range: Range<usize>, s: &str) {
        if range.len() == s.len() {
            self.buffer[range.clone()].copy_from_slice(s.as_bytes());
            if self.is_ascii {
                self.is_ascii = s.is_ascii();
            }
            return;
        }
        let new_len = self.buffer.len() + s.len() - range.len();
        self.buffer.resize(new_len, 0);
        self.buffer.copy_within(range.end.., range.start + s.len());
        self.buffer[range.start..range.start + s.len()].copy_from_slice(s.as_bytes());
        if self.is_ascii {
            self.is_ascii = s.is_ascii();
        }
    }
    fn replace_back_with_str(&mut self, range: RangeFrom<usize>, s: &str) {
        self.buffer.truncate(range.start);
        self.buffer.extend_from_slice(s.as_bytes());
        if self.is_ascii {
            self.is_ascii = s.is_ascii();
        }
    }
}

#[derive(Clone, Default)]
pub struct SourceBuf {
    lines: Vec<SourceLine>,
}

impl Display for SourceBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                f.write_str("\n")?;
            }
            f.write_str(line.as_str())?;
        }
        Ok(())
    }
}
impl Debug for SourceBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list()
            .entries(self.lines.iter().map(|line| line.as_str()))
            .finish()
    }
}

impl std::fmt::Write for SourceBuf {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        let mut lines = s.lines();
        let Some(begin_line) = lines.next() else {
            // no new line, just return
            return Ok(());
        };
        match self.lines.last_mut() {
            Some(last_line) => last_line.extend_with_str(begin_line),
            None => self
                .lines
                .push(SourceLine::from_linebuf(begin_line.as_bytes())),
        }
        for line in lines {
            self.lines.push(SourceLine::from_linebuf(line.as_bytes()));
        }
        Ok(())
    }
}

impl<'s> From<&'s str> for SourceBuf {
    fn from(src: &'s str) -> Self {
        let lines = src
            .lines()
            .map(|line| SourceLine::from_linebuf(line.as_bytes()))
            .collect();
        Self { lines }
    }
}
impl From<String> for SourceBuf {
    fn from(src: String) -> Self {
        Self::from(src.as_str())
    }
}

impl SourceBuf {
    pub fn lines(&self) -> &[SourceLine] {
        &self.lines
    }

    pub fn byte_pos_to_monaco(&self, pos: SourcePosIndex) -> Result<MonacoSrcPos, JsError> {
        let SourcePosIndex { line, col_byte } = pos;
        let Some(linebuf) = self.lines.get(line as usize) else {
            let line_count = self.lines.len();
            return fmt_jserr!(Err "line {line} exceeds total lines {line_count}");
        };
        let column = linebuf.byte_col_to_utf16(col_byte)?;
        Ok(MonacoSrcPos {
            line: line + 1,
            column: column + 1,
        })
    }
    pub fn monaco_pos_to_byte(&self, pos: MonacoSrcPos) -> Result<SourcePosIndex, JsError> {
        let MonacoSrcPos { line, column } = pos;
        let line = line.saturating_sub(1);
        let column = column.saturating_sub(1);
        let Some(linebuf) = self.lines.get(line as usize) else {
            let line_count = self.lines.len();
            return fmt_jserr!(Err "line {line} exceeds total lines {line_count}");
        };
        let col_byte = linebuf.utf16_col_to_byte(column)?;
        Ok(SourcePosIndex { line, col_byte })
    }
    pub fn byte_range_to_monaco(
        &self,
        range: Range<SourcePosIndex>,
    ) -> Result<Range<MonacoSrcPos>, JsError> {
        let start = self.byte_pos_to_monaco(range.start)?;
        let end = self.byte_pos_to_monaco(range.end)?;
        Ok(start..end)
    }

    pub fn replace(&mut self, range: SourceRangeIndex, s: &str) -> Result<(), JsError> {
        if s.is_empty() && range.is_empty() {
            return Ok(());
        }
        self.assert_range_inside(range.clone())?;
        SourceBufUpdateBuilder::new(self, range, s)?.build()
    }

    fn assert_range_inside(&self, range: SourceRangeIndex) -> Result<(), JsError> {
        use std::ops::Range; // 左闭右开区间, end 可以越过最后一行的末尾
        let Range { start, end } = range;
        if start > end {
            return fmt_jserr!(Err "invalid range: start {start:?} is after end {end:?}");
        }
        let (start_line, start_col) = (start.line as usize, start.col_byte as usize);
        let (end_line, end_col) = (end.line as usize, end.col_byte as usize);
        let Some(start_line_buf) = self.lines.get(start_line) else {
            return fmt_jserr!(Err "start line {start_line} exceeds total lines {line_count}", line_count = self.lines.len());
        };
        let Some(end_line_buf) = self.lines.get(end_line) else {
            if end_line == self.lines.len() && end_col == 0 {
                // allow end position to be at the end of the last line
                return Ok(());
            } else {
                return fmt_jserr!(Err "end line {end_line} exceeds total lines {line_count}", line_count = self.lines.len());
            }
        };
        let start_line_len = start_line_buf.buffer.len();
        let end_line_len = end_line_buf.buffer.len();
        if start_col > start_line_len {
            fmt_jserr!(Err "start column {start_col} exceeds line length {start_line_len} of line {start_line}")
        } else if !start_line_buf.as_str().is_char_boundary(start_col) {
            fmt_jserr!(Err "start column {start_col} of line {start_line} is in the middle of a UTF-8 character")
        } else if end_col > end_line_len {
            fmt_jserr!(Err "end column {end_col} exceeds line length {end_line_len} of line {end_line}")
        } else if !end_line_buf.as_str().is_char_boundary(end_col) {
            fmt_jserr!(Err "end column {end_col} of line {end_line} is in the middle of a UTF-8 character")
        } else {
            Ok(())
        }
    }
}

struct SourceBufUpdateBuilder<'buf, 'src> {
    buf: &'buf mut SourceBuf,
    start_line: usize,
    start_col: usize,
    end_linecol: Option<(usize, usize)>,
    new_text: &'src str,
}
impl<'buf, 'src> SourceBufUpdateBuilder<'buf, 'src> {
    fn new(
        buf: &'buf mut SourceBuf,
        range: SourceRangeIndex,
        new_text: &'src str,
    ) -> Result<Self, JsError> {
        let Range { start, end } = range;
        let (start_line, start_col) = (start.line as usize, start.col_byte as usize);
        let (end_line, end_col) = (end.line as usize, end.col_byte as usize);
        buf.assert_range_inside(range.clone())?;

        let end_linecol = if end_line >= buf.lines.len() {
            None
        } else if end_line + 1 == buf.lines.len() && end_col == buf.lines[end_line].buffer.len() {
            // allow end position to be at the end of the last line
            None
        } else {
            Some((end_line, end_col))
        };
        Ok(Self {
            buf,
            start_line,
            start_col,
            end_linecol,
            new_text,
        })
    }

    fn build(&mut self) -> Result<(), JsError> {
        if self.replace_single() {
            return Ok(());
        }

        let prefix = {
            let line = &self.buf.lines[self.start_line];
            &line.as_str()[..self.start_col]
        };
        let suffix = match self.end_linecol {
            Some((end_line, end_col)) => {
                let line = &self.buf.lines[end_line];
                &line.as_str()[end_col..]
            }
            None => "",
        };

        let mut replacement: Vec<SourceLine> = self
            .new_text
            .lines()
            .map(|s| SourceLine::from_linebuf(s.as_bytes()))
            .collect();

        if let Some(first_line) = replacement.first_mut() {
            if !prefix.is_empty() {
                let mut new_buf = SmallVec::from_slice(prefix.as_bytes());
                new_buf.extend_from_slice(&first_line.buffer);
                first_line.buffer = new_buf;
                first_line.is_ascii = first_line.buffer.iter().all(|&b| b.is_ascii());
            }
            first_line.is_ascii = first_line.is_ascii && prefix.is_ascii();

            if let Some(last_line) = replacement.last_mut() {
                last_line.extend_with_str(suffix);
            }
        } else {
            let mut merged = SourceLine::from_linebuf(prefix.as_bytes());
            merged.extend_with_str(suffix);
            replacement.push(merged);
        }

        let remove_end = match self.end_linecol {
            Some((end_line, _)) => end_line + 1,
            None => self.buf.lines.len(),
        };
        self.buf
            .lines
            .splice(self.start_line..remove_end, replacement);

        Ok(())
    }

    fn replace_single(&mut self) -> bool {
        match (self.start_line, self.end_linecol) {
            (start_line, Some((end_line, end_col))) if start_line == end_line => {
                let line = self.buf.lines.get_mut(start_line).unwrap();
                line.replace_with_str(self.start_col..end_col, self.new_text);
                true
            }
            (start_line, None) if start_line + 1 >= self.buf.lines.len() => {
                // appending at the end of the last line
                let line = self.buf.lines.last_mut().unwrap();
                line.replace_back_with_str(self.start_col.., self.new_text);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SourcePosIndex;

    #[test]
    fn replace_within_single_line() {
        let mut buf = SourceBuf::from("abcd\nef");
        let range = SourcePosIndex::new(0, 1)..SourcePosIndex::new(0, 3);
        buf.replace(range, "ZZ").unwrap();
        assert_eq!(buf.lines()[0].as_str(), "aZZd");
        assert_eq!(buf.lines()[1].as_str(), "ef");
    }

    #[test]
    fn replace_across_multiple_lines() {
        let mut buf = SourceBuf::from("hello\nworld\n!");
        let range = SourcePosIndex::new(0, 2)..SourcePosIndex::new(2, 0);
        buf.replace(range, "X\nY").unwrap();
        assert_eq!(buf.lines().len(), 2);
        assert_eq!(buf.lines()[0].as_str(), "heX");
        assert_eq!(buf.lines()[1].as_str(), "Y!");
    }

    #[test]
    #[should_panic(expected = "middle of a UTF-8 character")]
    fn replace_rejects_non_char_boundary() {
        let mut buf = SourceBuf::from("a中b");
        let range = SourcePosIndex::new(0, 2)..SourcePosIndex::new(0, 2);
        let _ = buf.replace(range, "x");
    }
}
