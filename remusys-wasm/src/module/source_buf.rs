use smallvec::SmallVec;
use smol_str::SmolStr;
use std::{io::Write, ops::Range};

use crate::source_tree::IRSrcTreePos;

type ByteBuf = SmallVec<[u8; 16]>;

#[derive(Debug, Clone, Default)]
pub struct IRSourceBuf {
    lines: Vec<ByteBuf>,
}

impl Write for IRSourceBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut start = 0;
        if let Err(u8err) = str::from_utf8(buf) {
            use std::io::{Error, ErrorKind};
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("invalid utf-8 data: {}", u8err),
            ));
        }
        for (i, &b) in buf.iter().enumerate() {
            if b == b'\n' {
                self.lines.push(buf[start..i].into());
                start = i + 1;
            }
        }
        if start < buf.len() {
            self.lines.push(buf[start..].into());
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::str::FromStr for IRSourceBuf {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut buf = IRSourceBuf::default();
        buf.write_all(s.as_bytes())
            .expect("writing to SourceBuf should not fail");
        Ok(buf)
    }
}

impl IRSourceBuf {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dump_smolstr(&self) -> SmolStr {
        if self.lines.is_empty() {
            return SmolStr::default();
        }
        let num_bytes = self.byte_count();
        let mut strs = ByteBuf::with_capacity(num_bytes);
        for str in &self.lines {
            strs.extend_from_slice(str.as_slice());
            strs.push(b'\n');
        }
        // SAFETY: SourceBuf only accepts valid UTF-8 data, so this is safe.
        let s = unsafe { str::from_utf8_unchecked(strs.as_slice()) };
        SmolStr::new(s)
    }

    pub fn lines(&self) -> &[ByteBuf] {
        self.lines.as_slice()
    }
    pub fn index_get_line(&self, line_index: usize) -> Option<&str> {
        self.lines().get(line_index).map(|line| unsafe {
            // SAFETY: SourceBuf only accepts valid UTF-8 data, so this is safe.
            str::from_utf8_unchecked(line.as_slice())
        })
    }
    pub fn order_get_line(&self, line_order: usize) -> Option<&str> {
        line_order
            .checked_sub(1)
            .and_then(|line_index| self.index_get_line(line_index))
    }
    pub fn line_count(&self) -> usize {
        self.lines.len()
    }
    pub fn byte_count(&self) -> usize {
        self.lines.iter().map(ByteBuf::len).sum::<usize>() + self.lines.len().saturating_sub(1)
    }

    pub fn index_take_line(&mut self, line_index: usize) -> Option<String> {
        self.lines.get(line_index).map(|line| unsafe {
            // SAFETY: SourceBuf only accepts valid UTF-8 data, so this is safe.
            str::from_utf8_unchecked(line.as_slice()).to_string()
        })
    }
    pub fn index_set_line(&mut self, line_index: usize, new_line: &str) -> Option<()> {
        if let Some(line) = self.lines.get_mut(line_index) {
            *line = new_line.as_bytes().into();
            Some(())
        } else {
            None
        }
    }

    pub fn end_pos(&self) -> IRSrcTreePos {
        IRSrcTreePos {
            line: self.lines.len() as u32,
            col_u16: self.end_col(),
        }
    }
    fn end_col(&self) -> u32 {
        match self.lines.last() {
            None => 0u32,
            Some(line) => unsafe {
                str::from_utf8_unchecked(line.as_slice())
                    .chars()
                    .map(|c| c.len_utf16())
                    .sum::<usize>() as u32
            },
        }
    }

    pub fn apply_line_update(&mut self, line_range: Range<usize>, lines: IRSourceBuf) {
        let start = line_range.start;
        let end = line_range.end;

        // 验证范围：start 不能大于 end
        if start > end {
            return; // 无效范围，静默返回
        }
        // 如果 start 超过当前行数，直接在末尾追加
        if start >= self.lines.len() {
            self.lines.extend(lines.lines);
            return;
        }
        // 限制 end 不超过当前行数
        let effective_end = end.min(self.lines.len());
        self.lines.splice(start..effective_end, lines.lines);
    }
}
