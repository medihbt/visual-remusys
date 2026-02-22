use logos::Span;
use remusys_ir::ir::{BlockID, GlobalID, InstID, JumpTargetID, UseID};
#[derive(Debug, Clone, Default)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IRSourceMapping {
    pub uses: Vec<(Span, UseID)>,
    pub jts: Vec<(Span, JumpTargetID)>,
    pub blocks: Vec<(Span, BlockID)>,
    pub insts: Vec<(Span, InstID)>,
    pub gvars: Vec<(Span, GlobalID)>,
    pub funcs: Vec<IRFuncSrcMapping>,
}

#[derive(Debug, Clone)]
// #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct IRFuncSrcMapping {
    pub head_span: Span,
    pub full_span: Span,
    pub id: GlobalID,
    pub args: Vec<Span>,
}
