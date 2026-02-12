use logos::Span;
use remusys_ir::ir::{BlockIndex, GlobalIndex, InstIndex, JumpTargetIndex, UseIndex};

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IRSourceMapping {
    pub uses: Vec<(Span, UseIndex)>,
    pub jts: Vec<(Span, JumpTargetIndex)>,
    pub blocks: Vec<(Span, BlockIndex)>,
    pub insts: Vec<(Span, InstIndex)>,
    pub gvars: Vec<(Span, GlobalIndex)>,
    pub funcs: Vec<IRFuncSrcMapping>,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IRFuncSrcMapping {
    pub head_span: Span,
    pub full_span: Span,
    pub id: GlobalIndex,
    pub args: Vec<Span>,
}
