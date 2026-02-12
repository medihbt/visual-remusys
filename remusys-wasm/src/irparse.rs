use std::ops::Range;

use remusys_ir::ir::{BlockIndex, GlobalIndex, InstIndex, JumpTargetIndex, UseIndex};
use remusys_ir_parser::mapping::{IRFuncSrcMapping, IRSourceMapping};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsSourceLoc {
    /// the begin line of the source location (1-based)
    pub begin_line: usize,
    /// the begin column of the source location in UTF-16 code units (0-based)
    pub begin_col: usize,
    /// the end line of the source location (1-based)
    pub end_line: usize,
    /// the end column of the source location in UTF-16 code units (0-based)
    pub end_col: usize,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SourceMapErr {
    #[error("line {line} is out of bounds (max line: {max_line})")]
    LineOutOfBounds { line: usize, max_line: usize },

    #[error("column {col} is out of bounds (max column: {max_col})")]
    ColumnOutOfBounds { col: usize, max_col: usize },
}
pub type SourceMapRes<T = ()> = Result<T, SourceMapErr>;

struct JsSourceMapBuilder<'src> {
    src: &'src str,
    line_u8pos_map: Vec<usize>,
}

impl<'src> JsSourceMapBuilder<'src> {
    pub fn new(src: &'src str) -> Self {
        let mut line_u8pos_map = vec![0];
        for (i, c) in src.char_indices() {
            if c == '\n' {
                line_u8pos_map.push(i + 1);
            }
        }
        Self {
            src,
            line_u8pos_map,
        }
    }

    fn map_u8pos(&self, u8pos: usize) -> SourceMapRes<(usize, usize)> {
        if u8pos > self.src.len() {
            return Err(SourceMapErr::LineOutOfBounds {
                line: self.line_u8pos_map.len(),
                max_line: self.line_u8pos_map.len(),
            });
        }
        let line = match self.line_u8pos_map.binary_search(&u8pos) {
            Ok(line) => line,
            Err(line) => line - 1,
        };
        let line_source = &self.src[self.line_u8pos_map[line]..u8pos];
        let col = line_source.encode_utf16().count();
        Ok((line + 1, col))
    }
    pub fn map_u8span(&self, span: Range<usize>) -> SourceMapRes<JsSourceLoc> {
        let (begin_line, begin_col) = self.map_u8pos(span.start)?;
        let (end_line, end_col) = self.map_u8pos(span.end)?;
        Ok(JsSourceLoc {
            begin_line,
            begin_col,
            end_line,
            end_col,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRSourceMappingDt {
    pub source: SmolStr,
    pub uses: Vec<(JsSourceLoc, UseIndex)>,
    pub jts: Vec<(JsSourceLoc, JumpTargetIndex)>,
    pub blocks: Vec<(JsSourceLoc, BlockIndex)>,
    pub insts: Vec<(JsSourceLoc, InstIndex)>,
    pub gvars: Vec<(JsSourceLoc, GlobalIndex)>,
    pub funcs: Vec<IRFuncSrcMappingDt>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IRFuncSrcMappingDt {
    pub head_loc: JsSourceLoc,
    pub full_loc: JsSourceLoc,
    pub id: GlobalIndex,
    pub args: Vec<JsSourceLoc>,
}

impl IRSourceMappingDt {
    pub fn from_mapping(src: &str, mapping: IRSourceMapping) -> Self {
        let builder = JsSourceMapBuilder::new(src);
        fn map_dt<T: Clone>(
            builder: &JsSourceMapBuilder,
            items: &[(Range<usize>, T)],
        ) -> Vec<(JsSourceLoc, T)> {
            items
                .iter()
                .map(|(span, idx)| (builder.map_u8span(span.clone()).unwrap(), idx.clone()))
                .collect()
        }
        fn map_func(
            builder: &JsSourceMapBuilder,
            func: &[IRFuncSrcMapping],
        ) -> Vec<IRFuncSrcMappingDt> {
            func.iter()
                .map(|f| IRFuncSrcMappingDt {
                    head_loc: builder.map_u8span(f.head_span.clone()).unwrap(),
                    full_loc: builder.map_u8span(f.full_span.clone()).unwrap(),
                    id: f.id,
                    args: f
                        .args
                        .iter()
                        .map(|span| builder.map_u8span(span.clone()).unwrap())
                        .collect(),
                })
                .collect()
        }
        Self {
            source: src.into(),
            uses: map_dt(&builder, &mapping.uses),
            jts: map_dt(&builder, &mapping.jts),
            blocks: map_dt(&builder, &mapping.blocks),
            insts: map_dt(&builder, &mapping.insts),
            gvars: map_dt(&builder, &mapping.gvars),
            funcs: map_func(&builder, &mapping.funcs),
        }
    }
}
