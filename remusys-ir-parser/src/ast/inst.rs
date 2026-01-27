use remusys_ir::ir::{CmpCond, inst::BinOPFlags};
use smol_str::SmolStr;
use std::fmt::Debug;

use crate::{
    ast::{AstNode, Ident, IdentKind, Label, Operand, TypeAst, TypeAstKind, TypeValue, utils},
    parse_err,
    parser::{IRParseRes, IRParser},
    tokens::FinalToken,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InstSection {
    Phi,
    Inst,
    Terminator,
}

#[derive(Debug)]
pub enum InstKind {
    // basic block terminators
    /// Syntax: `unreachable`
    Unreachable,

    /// Syntax: `ret void`
    RetVoid,

    /// Syntax: `ret <ty> <val>`
    Ret(RetAst),

    /// Syntax: `br label <label>`
    Jump(Label),

    /// Syntax: `br i1 <cond>, label <then>, label <else>`
    Br(BrAst),

    /// Syntax: see `SwitchAst`
    Switch(SwitchAst),

    /// phi nodes
    Phi(PhiAst),

    // memory operation
    /// Syntax: `alloca <ty>[, align <align>]?`
    Alloca(AllocaAst),

    /// Syntax:
    ///
    /// ```remusys_ir
    /// getelementptr <init_ty>, <ptr_ty> <init_ptr>, <ty> <index>, <ty> <index> ...
    /// ```
    GEP(GEPAst),

    /// Syntax:
    ///
    /// ```remusys_ir
    /// load <ty>, <ptr_ty> <ptr>[, align <align>]?
    /// ```
    Load(LoadAst),

    /// Syntax:
    ///
    /// ```remusys_ir
    /// store <ty> <val>, <ptr_ty> <ptr>[, align <align>]?
    /// ```
    Store(StoreAst),

    // data processing instructions
    /// Syntax: `<binop> <ty> <lhs>, <rhs>`
    Bin(BinAst),

    /// Syntax: `<castop> <ty> <val> to <dest_ty>`
    Cast(CastAst),

    /// Syntax: `<cmpop> <cond> <ty> <lhs>, <rhs>`
    Cmp(CmpAst),

    /// Syntax: `select <cond_ty> <cond>, <ty> <then_val>, <else_val>`
    Select(SelectAst),

    /// Syntax: `[tail]? call <ret_ty> ['(' '...'] <func_val>(<args>) ')'
    Call(CallAst),
}
pub struct InstAst {
    pub kind: InstKind,
    pub span: logos::Span,
    id: Option<Ident>,
}

impl Debug for InstAst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { kind, span, id } = self;
        if let Some(id) = id {
            write!(f, "InstAst({span:?} = %{id:?})::")?;
        } else {
            write!(f, "InstAst({span:?})::")?;
        }
        match kind {
            InstKind::Unreachable => write!(f, "Unreachable"),
            InstKind::RetVoid => write!(f, "RetVoid"),
            InstKind::Ret(ret) => Debug::fmt(ret, f),
            InstKind::Jump(label) => write!(f, "Jump to {label:?}"),
            InstKind::Br(br) => Debug::fmt(br, f),
            InstKind::Switch(switch) => Debug::fmt(switch, f),
            InstKind::Phi(phi) => Debug::fmt(phi, f),
            InstKind::Alloca(alloca) => Debug::fmt(alloca, f),
            InstKind::GEP(gep) => Debug::fmt(gep, f),
            InstKind::Load(load) => Debug::fmt(load, f),
            InstKind::Store(store) => Debug::fmt(store, f),
            InstKind::Bin(bin) => Debug::fmt(bin, f),
            InstKind::Cast(cast) => Debug::fmt(cast, f),
            InstKind::Cmp(cmp) => Debug::fmt(cmp, f),
            InstKind::Select(select) => Debug::fmt(select, f),
            InstKind::Call(call) => Debug::fmt(call, f),
        }
    }
}

impl AstNode for InstAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.parser_pos();
        Self::do_parse(parser).map_err(|e| e.map_span(|s| begin_pos..s.end))
    }
}

impl InstAst {
    pub fn get_id(&self) -> Option<&Ident> {
        self.id.as_ref()
    }
    pub fn set_id(&mut self, id: Ident) {
        self.span.start = id.span.start;
        self.id = Some(id);
    }

    pub fn get_section(&self) -> InstSection {
        match &self.kind {
            InstKind::Phi(_) => InstSection::Phi,
            InstKind::Unreachable
            | InstKind::RetVoid
            | InstKind::Ret(_)
            | InstKind::Jump(_)
            | InstKind::Br(_)
            | InstKind::Switch(_) => InstSection::Terminator,
            _ => InstSection::Inst,
        }
    }

    fn do_parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let (id, begin_pos) = Self::parse_head(parser)?;
        let (opcode, opspan) = match parser.peek0()? {
            (FinalToken::Word(opcode), span) => (opcode, span),
            (tok, span) => {
                return parse_err!(Unmatch span, "opcode requires word but got token {tok:?}");
            }
        };

        fn inst<T>(t: T, id: Option<Ident>) -> InstAst
        where
            InstAst: From<T>,
        {
            let mut inst = InstAst::from(t);
            if let Some(id) = id {
                inst.set_id(id);
            }
            inst
        }

        match opcode.as_str() {
            "unreachable" => {
                parser.advance_n(1)?;
                Ok(Self {
                    kind: InstKind::Unreachable,
                    span: begin_pos..opspan.end,
                    id,
                })
            }
            "ret" => Self::parse_ret(parser).map(|s| inst(s, id)),
            "br" => Self::parse_br(parser).map(|s| inst(s, id)),
            "switch" => SwitchAst::parse(parser).map(|s| inst(s, id)),

            "phi" => PhiAst::parse(parser).map(|s| inst(s, id)),

            "alloca" => AllocaAst::parse(parser).map(|s| inst(s, id)),
            "getelementptr" => GEPAst::parse(parser).map(|s| inst(s, id)),
            "load" => LoadAst::parse(parser).map(|s| inst(s, id)),
            "store" => StoreAst::parse(parser).map(|s| inst(s, id)),

            "add" | "sub" | "mul" | "sdiv" | "udiv" | "srem" | "urem" | "fadd" | "fsub"
            | "fmul" | "fdiv" | "frem" | "shl" | "lshr" | "ashr" | "and" | "or" | "xor" => {
                BinAst::parse(parser).map(|s| inst(s, id))
            }

            "zext" | "sext" | "trunc" | "fpext" | "fptrunc" | "bitcast" | "sitofp" | "uitofp"
            | "fptosi" | "fptoui" | "inttoptr" | "ptrtoint" => {
                CastAst::parse(parser).map(|s| inst(s, id))
            }

            "icmp" | "fcmp" => CmpAst::parse(parser).map(|s| inst(s, id)),

            "select" => SelectAst::parse(parser).map(|s| inst(s, id)),

            "call" | "tail" => CallAst::parse(parser).map(|s| inst(s, id)),

            "extractvalue" => todo!("parse field extract"),
            "extractelement" => todo!("parse index extract"),
            "insertvalue" => todo!("parse field insert"),
            "insertelement" => todo!("parse index insert"),

            opcode => {
                parse_err!(Unmatch begin_pos..opspan.end, "Unrecognized opcode `{opcode}`")
            }
        }
    }
    fn parse_head(parser: &mut IRParser<'_>) -> IRParseRes<(Option<Ident>, usize)> {
        let (tok0, span0) = parser.peek0()?;
        let begin_pos = span0.start;
        let id = if let FinalToken::PIdent(id) = tok0 {
            parser.advance_n(1)?;
            parser
                .advance_exact(&[FinalToken::Eq])
                .map_err(|e| e.map_span(|s| begin_pos..s.end))?;
            Some(Ident {
                kind: IdentKind::Local,
                span: span0,
                name: id,
            })
        } else {
            None
        };
        Ok((id, begin_pos))
    }

    fn parse_ret(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.parser_pos();
        let (tok, span) = parser.peek1()?;
        if tok.is_word("void") {
            parser.advance_n(2)?;
            Ok(Self {
                span: begin_pos..span.end,
                id: None,
                kind: InstKind::RetVoid,
            })
        } else {
            RetAst::parse(parser).map(Self::from)
        }
    }
    fn parse_br(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.parser_pos();
        let (tok, _) = parser.peek1()?;
        if tok.is_word("label") {
            parser.advance_n(1)?;
            let target = Label::parse(parser)?;
            let span = begin_pos..target.span.end;
            Ok(Self {
                kind: InstKind::Jump(target),
                span,
                id: None,
            })
        } else {
            BrAst::parse(parser).map(Self::from)
        }
    }
}

#[derive(Debug)]
pub struct RetAst {
    pub span: logos::Span,
    pub tyval: TypeValue,
}
impl From<RetAst> for InstAst {
    fn from(value: RetAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Ret(value),
            span,
            id: None,
        }
    }
}
impl AstNode for RetAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.advance_exact(&[FinalToken::lit_word("ret")])?.start;
        let tyval = TypeValue::parse(parser)?;
        Ok(Self {
            span: begin_pos..tyval.get_span().end,
            tyval,
        })
    }
}

#[derive(Debug)]
pub struct BrAst {
    pub span: logos::Span,
    pub cond: TypeValue,
    pub then_bb: Label,
    pub else_bb: Label,
}
impl From<BrAst> for InstAst {
    fn from(value: BrAst) -> Self {
        let span = value.span.clone();
        InstAst {
            kind: InstKind::Br(value),
            span,
            id: None,
        }
    }
}
impl AstNode for BrAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.advance_exact(&[FinalToken::lit_word("br")])?.start;
        let cond = TypeValue::parse(parser)?;
        let TypeAstKind::Int(1) = &cond.ty.kind else {
            return parse_err!(InstErrInvalidCondType begin_pos..cond.get_span().end);
        };
        parser.advance_exact(&[FinalToken::Comma])?;
        let then_bb = Label::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let else_bb = Label::parse(parser)?;

        Ok(BrAst {
            span: begin_pos..else_bb.span.end,
            cond,
            then_bb,
            else_bb,
        })
    }
}

#[derive(Debug)]
pub struct SwitchCase {
    pub discrim: TypeValue,
    pub label: Label,
}
impl AstNode for SwitchCase {
    fn get_span(&self) -> logos::Span {
        let start = self.discrim.get_span().start;
        let end = self.label.get_span().end;
        start..end
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let discrim = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let label = Label::parse(parser)?;
        Ok(Self { discrim, label })
    }
}

/// Syntax:
///
/// ```remusys_ir
/// switch <type> <cond>, label <default_bb>, [
///     <type> <case1>, label <bb1>
///     <type> <case2>, label <bb2>
/// ]
/// ```
#[derive(Debug)]
pub struct SwitchAst {
    pub span: logos::Span,
    pub cond: TypeValue,
    pub default_bb: Label,
    pub cases: Vec<SwitchCase>,
}
impl From<SwitchAst> for InstAst {
    fn from(value: SwitchAst) -> Self {
        let span = value.get_span();
        Self {
            kind: InstKind::Switch(value),
            span,
            id: None,
        }
    }
}
impl AstNode for SwitchAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser
            .advance_exact(&[FinalToken::lit_word("switch")])?
            .start;
        let cond = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;

        let default_bb = Label::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma, FinalToken::LBracket])?;

        let mut cases: Vec<SwitchCase> = Vec::new();
        loop {
            if parser.peek0_match(FinalToken::RBracket)?.0 {
                break;
            }
            cases.push(SwitchCase::parse(parser)?);
        }
        let end_pos = parser.advance_exact(&[FinalToken::RBracket])?.end;
        Ok(Self {
            span: begin_pos..end_pos,
            cond,
            default_bb,
            cases,
        })
    }
}

/// PHI node
/// Syntax:
///
/// ```remusys_ir
/// phi <type> [<value1>, %<label1>], [<value2>, <label2>], ...
/// ```
#[derive(Debug)]
pub struct PhiAst {
    pub span: logos::Span,
    pub ty: TypeAst,
    pub incomes: Vec<(Operand, Ident)>,
}
impl From<PhiAst> for InstAst {
    fn from(value: PhiAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Phi(value),
            span,
            id: None,
        }
    }
}
impl AstNode for PhiAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.advance_exact(&[FinalToken::lit_word("phi")])?.start;
        let ty = TypeAst::parse(parser)?;
        let mut incomes: Vec<(Operand, Ident)> = Vec::new();
        loop {
            match parser.peek0()? {
                (FinalToken::Comma, _) => {
                    parser.advance_exact(&[FinalToken::Comma, FinalToken::LBracket])?;
                }
                _ => break,
            }
            let operand = Operand::parse(parser)?;
            parser.advance_exact(&[FinalToken::Comma])?;
            let label = Ident::parse(parser)?;
            if label.kind != IdentKind::Local {
                return parse_err!(Unmatch label.get_span(), "Phi instruction label should be '%label'");
            }
            incomes.push((operand, label));
        }
        let end_pos = parser.parser_pos();
        Ok(Self {
            span: begin_pos..end_pos,
            ty,
            incomes,
        })
    }
}

/// Syntax:
///
/// ```remusys_ir
/// alloca <ty>
/// alloca <ty>, align <align>
/// ```
#[derive(Debug)]
pub struct AllocaAst {
    pub span: logos::Span,
    pub ty: TypeAst,
    pub align: Option<usize>,
}
impl From<AllocaAst> for InstAst {
    fn from(value: AllocaAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Alloca(value),
            span,
            id: None,
        }
    }
}
impl AstNode for AllocaAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser
            .advance_exact(&[FinalToken::lit_word("alloca")])?
            .start;
        let ty = TypeAst::parse(parser)?;
        let align = utils::parse_align(parser)?;
        let end_pos = parser.parser_pos();
        Ok(Self {
            span: begin_pos..end_pos,
            ty,
            align,
        })
    }
}

/// Syntax:
///
/// ```remusys_ir
/// getelementptr <init_ty>, <ptr_ty> <init_ptr>, <ty> <index>, <ty> <index> ...
/// ```
#[derive(Debug)]
pub struct GEPAst {
    pub span: logos::Span,
    pub inbounds: bool,
    pub init_ty: TypeAst,
    pub initptr: TypeValue,
    pub indices: Vec<TypeValue>,
}
impl From<GEPAst> for InstAst {
    fn from(value: GEPAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::GEP(value),
            span,
            id: None,
        }
    }
}
impl AstNode for GEPAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser
            .advance_exact(&[FinalToken::lit_word("getelementptr")])?
            .start;
        let inbounds = if parser.peek0_match(FinalToken::lit_word("inbounds"))?.0 {
            parser.advance_n(1)?;
            true
        } else {
            false
        };
        let init_ty = TypeAst::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;

        let initptr = TypeValue::parse(parser)?;
        let mut indices: Vec<TypeValue> = Vec::new();

        loop {
            if let (FinalToken::Comma, _) = parser.peek0()? {
                break;
            };
            parser.advance_n(1)?;
            indices.push(TypeValue::parse(parser)?);
        }

        let end_pos = parser.parser_pos();
        Ok(Self {
            span: begin_pos..end_pos,
            inbounds,
            init_ty,
            initptr,
            indices,
        })
    }
}

/// Syntax:
///
/// ```remusys_ir
/// load <ty>, <ptr_ty> <ptr>[, align <align>]?
/// ```
#[derive(Debug)]
pub struct LoadAst {
    pub span: logos::Span,
    pub ty: TypeAst,
    pub src: TypeValue,
    pub align: Option<usize>,
}
impl From<LoadAst> for InstAst {
    fn from(value: LoadAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Load(value),
            span,
            id: None,
        }
    }
}
impl AstNode for LoadAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.advance_exact(&[FinalToken::lit_word("load")])?.start;
        let ty = TypeAst::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let src = TypeValue::parse(parser)?;

        let align = utils::parse_align(parser)?;
        let end_pos = parser.parser_pos();
        Ok(LoadAst {
            span: begin_pos..end_pos,
            ty,
            src,
            align,
        })
    }
}

/// Syntax:
///
/// ```remusys_ir
/// store <ty> <val>, <ptr_ty> <ptr>[, align <align>]?
/// ```
#[derive(Debug)]
pub struct StoreAst {
    pub span: logos::Span,
    pub val: TypeValue,
    pub dest: TypeValue,
    pub align: Option<usize>,
}
impl From<StoreAst> for InstAst {
    fn from(value: StoreAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Store(value),
            span,
            id: None,
        }
    }
}
impl AstNode for StoreAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser
            .advance_exact(&[FinalToken::lit_word("store")])?
            .start;
        let val = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let dest = TypeValue::parse(parser)?;

        let align = utils::parse_align(parser)?;
        let end_pos = parser.parser_pos();
        Ok(StoreAst {
            span: begin_pos..end_pos,
            val,
            dest,
            align,
        })
    }
}

/// Syntax:
///
/// ```remusys_ir
/// <binop> [flag]* <ty> <lhs>, <rhs>
///
/// flag: 'nsw' | 'nuw' | 'exact'
/// ```
pub struct BinAst {
    pub span: logos::Span,
    pub op: SmolStr,
    pub flags: BinOPFlags,
    pub lhs: TypeValue,
    pub rhs: Operand,
}
impl Debug for BinAst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BinAst")
            .field("span", &self.span)
            .field("op", &self.op)
            .field("flags", &self.flags.as_str())
            .field("lhs", &self.lhs)
            .field("rhs", &self.rhs)
            .finish()
    }
}
impl From<BinAst> for InstAst {
    fn from(value: BinAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Bin(value),
            span,
            id: None,
        }
    }
}
impl AstNode for BinAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.parser_pos();
        let (tok, span) = parser.peek0()?;
        let op = if let FinalToken::Word(op) = tok {
            parser.advance_n(1)?;
            op
        } else {
            return parse_err!(Unmatch span, "binary operation requires word opcode");
        };
        let flags = Self::parse_flags(parser)?;
        let lhs = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let rhs = Operand::parse(parser)?;
        let end_pos = parser.parser_pos();
        Ok(Self {
            span: begin_pos..end_pos,
            flags,
            op,
            lhs,
            rhs,
        })
    }
}
impl BinAst {
    fn parse_flags(parser: &mut IRParser<'_>) -> IRParseRes<BinOPFlags> {
        let mut flags = BinOPFlags::NONE;
        loop {
            let (tok, _) = parser.peek0()?;
            let FinalToken::Word(word) = tok else {
                break;
            };
            let flag = match word.as_str() {
                "nsw" => BinOPFlags::NSW,
                "nuw" => BinOPFlags::NUW,
                "exact" => BinOPFlags::EXACT,
                _ => break,
            };
            parser.advance_n(1)?;
            flags.insert(flag);
        }
        Ok(flags)
    }
}

/// Syntax:
///
/// ```remusys_ir
/// <castop> <type> <val> to <dest_ty>
///
/// castop: 'zext' | 'sext' | 'trunc'
///       | 'fpext' | 'fptrunc' | 'bitcast'
///       | 'sitofp' | 'uitofp' | 'fptosi' | 'fptoui'
///       | 'inttoptr' | 'ptrtoint'
/// ```
#[derive(Debug)]
pub struct CastAst {
    pub span: logos::Span,
    pub op: SmolStr,
    pub tyval: TypeValue,
    pub dest_ty: TypeAst,
}
impl From<CastAst> for InstAst {
    fn from(value: CastAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Cast(value),
            span,
            id: None,
        }
    }
}
impl AstNode for CastAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.parser_pos();
        let (tok, span) = parser.peek0()?;
        let op = if let FinalToken::Word(op) = tok {
            parser.advance_n(1)?;
            op
        } else {
            return parse_err!(Unmatch span, "cast operation requires word opcode");
        };
        let tyval = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::lit_word("to")])?;
        let dest_ty = TypeAst::parse(parser)?;
        let end_pos = parser.parser_pos();
        Ok(Self {
            span: begin_pos..end_pos,
            op,
            tyval,
            dest_ty,
        })
    }
}

pub struct CmpAst {
    pub span: logos::Span,
    pub op: SmolStr,
    pub cond: CmpCond,
    pub lhs: TypeValue,
    pub rhs: Operand,
}
impl Debug for CmpAst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CmpAst")
            .field("span", &self.span)
            .field("cond", &self.cond.as_str())
            .field("lhs", &self.lhs)
            .field("rhs", &self.rhs)
            .finish()
    }
}
impl From<CmpAst> for InstAst {
    fn from(value: CmpAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Cmp(value),
            span,
            id: None,
        }
    }
}
impl AstNode for CmpAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let (opcode, begin_pos) = Self::parse_head(parser)?;
        // parse condition
        let mut cond = Self::parse_cond(parser)?;
        let lhs = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let rhs = Operand::parse(parser)?;
        let end_pos = parser.parser_pos();

        if opcode == "fcmp" {
            cond.insert(CmpCond::FLOAT_SWITCH);
        }
        Ok(Self {
            span: begin_pos..end_pos,
            op: opcode,
            cond,
            lhs,
            rhs,
        })
    }
}
impl CmpAst {
    fn parse_cond(parser: &mut IRParser<'_>) -> IRParseRes<CmpCond> {
        let (tok, span) = parser.peek0()?;
        let FinalToken::Word(cond) = tok else {
            return parse_err!(Unmatch span, "compare condition requires word");
        };
        parser.advance_n(1)?;
        let cond = match cond.as_str() {
            "eq" => CmpCond::EQ,
            "ne" => CmpCond::NE,
            "ugt" => CmpCond::GT,
            "uge" => CmpCond::GE,
            "ult" => CmpCond::LT,
            "ule" => CmpCond::LE,
            "sgt" => CmpCond::SGT,
            "sge" => CmpCond::SGE,
            "slt" => CmpCond::SLT,
            "sle" => CmpCond::SLE,
            "ogt" => CmpCond::SGT | CmpCond::FLOAT_SWITCH,
            "olt" => CmpCond::SLT | CmpCond::FLOAT_SWITCH,
            "oge" => CmpCond::SGE | CmpCond::FLOAT_SWITCH,
            "ole" => CmpCond::SLE | CmpCond::FLOAT_SWITCH,
            "false" => CmpCond::NEVER,
            "true" => CmpCond::ALWAYS,
            _ => return parse_err!(Unmatch span, "invalid compare condition `{cond}`"),
        };
        Ok(cond)
    }

    fn parse_head(parser: &mut IRParser<'_>) -> IRParseRes<(SmolStr, usize)> {
        let (tok, span) = parser.peek0()?;
        let begin_pos = span.start;
        let opcode = if let FinalToken::Word(opcode) = tok {
            parser.advance_n(1)?;
            opcode
        } else {
            return parse_err!(Unmatch span, "compare operation requires word opcode");
        };
        Ok((opcode, begin_pos))
    }
}

/// Syntax:
///
/// ```remusys_ir
/// select <cond_ty> <cond>, <ty> <then_val>, <else_val>
/// ```
#[derive(Debug)]
pub struct SelectAst {
    pub span: logos::Span,
    pub cond: TypeValue,
    pub then_val: TypeValue,
    pub else_val: Operand,
}
impl From<SelectAst> for InstAst {
    fn from(value: SelectAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Select(value),
            span,
            id: None,
        }
    }
}
impl AstNode for SelectAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser
            .advance_exact(&[FinalToken::lit_word("select")])?
            .start;
        let cond = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let then_val = TypeValue::parse(parser)?;
        parser.advance_exact(&[FinalToken::Comma])?;
        let else_val = Operand::parse(parser)?;
        let end_pos = parser.parser_pos();
        Ok(Self {
            span: begin_pos..end_pos,
            cond,
            then_val,
            else_val,
        })
    }
}

/// Syntax:
///
/// ```remusys_ir
/// [tail]? call <ret_ty> ['(' '...' ')']? <func_val>(<arg1_ty> <arg1>, <arg2_ty> <arg2>, ...)
/// ```
#[derive(Debug)]
pub struct CallAst {
    pub span: logos::Span,
    /// syntax unit: `tail` indicating tail call
    pub is_tail: bool,
    /// syntax unit: `(...)` indicating vararg function
    pub is_vararg: bool,
    pub ret_ty: TypeAst,
    /// not necessarily an identifier, could be function pointer
    pub func: Operand,
    pub args: Vec<TypeValue>,
}
impl From<CallAst> for InstAst {
    fn from(value: CallAst) -> Self {
        let span = value.span.clone();
        Self {
            kind: InstKind::Call(value),
            span,
            id: None,
        }
    }
}
impl AstNode for CallAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        use crate::tokens::FinalToken as T;
        let (is_tail, begin_pos) = if parser.peek0_match(T::lit_word("tail"))?.0 {
            let begin_pos = parser.advance_exact(&[T::lit_word("tail")])?.start;
            (true, begin_pos)
        } else {
            (false, parser.advance_exact(&[T::lit_word("call")])?.start)
        };
        let ret_ty = TypeAst::parse(parser)?;

        let is_vararg = if parser.peek0_match(T::LParen)?.0 {
            parser.advance_exact(&[T::LParen, T::Ellipsis, T::RParen])?;
            true
        } else {
            false
        };
        let func = Operand::parse(parser)?;
        let mut args: Vec<TypeValue> = Vec::new();

        parser.advance_exact(&[T::LParen])?;
        loop {
            if parser.peek0_match(T::RParen)?.0 {
                break;
            }
            if !args.is_empty() {
                parser.advance_exact(&[T::Comma])?;
            }
            let arg = TypeValue::parse(parser)?;
            args.push(arg);
        }
        let end_pos = parser.advance_exact(&[T::RParen])?.end;
        Ok(Self {
            span: begin_pos..end_pos,
            is_tail,
            is_vararg,
            ret_ty,
            func,
            args,
        })
    }
}
