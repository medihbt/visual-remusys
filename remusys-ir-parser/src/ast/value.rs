use std::{fmt::Debug, sync::Arc};

use crate::{
    ast::{AstNode, Ident, IdentKind, TypeAst},
    parse_err,
    parser::{IRParseRes, IRParser},
    tokens::FinalToken,
};
use smol_str::SmolStr;

#[derive(Debug, Clone)]
pub enum OperandKind {
    Undef,
    Poison,
    Zeroinit,
    Null,
    Bool(bool),
    Int(i128),
    FP(f64),
    Global(SmolStr),
    Local(SmolStr),
    Bytes(Arc<[u8]>),
    Aggr(Aggr),
    Sparse(SparseExpr),
}
#[derive(Clone)]
pub struct Operand {
    pub span: logos::Span,
    pub kind: OperandKind,
}
impl Debug for Operand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { span, kind } = self;
        match kind {
            OperandKind::Undef => write!(f, "Operand::Undef ({span:?})"),
            OperandKind::Poison => write!(f, "Operand::Poison ({span:?})"),
            OperandKind::Zeroinit => write!(f, "Operand::ZeroInit ({span:?})"),
            OperandKind::Null => write!(f, "Operand::Null ({span:?})"),
            OperandKind::Bool(b) => write!(f, "Operand::Bool ({span:?}: {b})"),
            OperandKind::Int(i) => write!(f, "Operand::Int ({span:?}: {i})"),
            OperandKind::FP(fp) => write!(f, "Operand::FP ({span:?}: {fp})"),
            OperandKind::Global(name) => write!(f, "Operand::Global({span:?}: @{name:?})"),
            OperandKind::Local(name) => write!(f, "Operand::Local({span:?}: %{name:?})"),
            OperandKind::Bytes(items) => write!(f, "Operand::Bytes ({span:?}: {items:?})"),
            OperandKind::Aggr(aggr) => Debug::fmt(aggr, f),
            OperandKind::Sparse(sparse) => Debug::fmt(sparse, f),
        }
    }
}
impl AstNode for Operand {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let (tok0, span0) = parser.peek0()?;
        match tok0 {
            FinalToken::Word(word) => {
                let kind = match word.as_str() {
                    "undef" => OperandKind::Undef,
                    "poison" => OperandKind::Poison,
                    "null" => OperandKind::Null,
                    "zeroinitialzier" => OperandKind::Zeroinit,
                    "false" => OperandKind::Bool(false),
                    "true" => OperandKind::Bool(true),
                    "sparse" => return SparseExpr::parse(parser).map(|s| s.into()),
                    word => {
                        return parse_err!(Unmatch span0, "value as word is unexpected {word:?}");
                    }
                };
                parser.advance_n(1)?;
                Ok(Self { span: span0, kind })
            }
            FinalToken::LitInt(li) => {
                parser.advance_n(1)?;
                Ok(Self {
                    span: span0,
                    kind: OperandKind::Int(li),
                })
            }
            FinalToken::LitFP(fp) => {
                parser.advance_n(1)?;
                Ok(Self {
                    span: span0,
                    kind: OperandKind::FP(fp),
                })
            }
            FinalToken::AIdent(id) => {
                parser.advance_n(1)?;
                Ok(Self {
                    span: span0,
                    kind: OperandKind::Global(id),
                })
            }
            FinalToken::PIdent(id) => {
                parser.advance_n(1)?;
                Ok(Self {
                    span: span0,
                    kind: OperandKind::Local(id),
                })
            }
            FinalToken::LitBytes(mut bytes) => {
                parser.advance_n(1)?;
                Ok(Self {
                    span: span0,
                    kind: OperandKind::Bytes(Arc::from(bytes.as_mut_slice())),
                })
            }
            FinalToken::LBracket | FinalToken::LBrace | FinalToken::LAngle => {
                Aggr::parse(parser).map(|a| a.into())
            }
            _ => parse_err!(Unmatch span0, "invalid syntax for Operand"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TypeValue {
    pub ty: TypeAst,
    pub val: Operand,
}

impl AstNode for TypeValue {
    fn get_span(&self) -> logos::Span {
        self.ty.span.start..self.val.span.end
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let ty = TypeAst::parse(parser)?;
        let val = Operand::parse(parser)?;
        Ok(Self { ty, val })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AggrKind {
    Array,
    Vec,
    Struct,
    PackStruct,
}

#[derive(Debug, Clone)]
pub struct Aggr {
    pub span: logos::Span,
    pub kind: AggrKind,
    pub elems: Vec<TypeValue>,
}
impl From<Aggr> for Operand {
    fn from(value: Aggr) -> Self {
        let span = value.span.clone();
        Operand {
            span,
            kind: OperandKind::Aggr(value),
        }
    }
}
impl AstNode for Aggr {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    /// ```remusys_ir
    /// array: '[' (TypeValue ',')* ']'
    /// struct: '{' (TypeValue ',')* '}'
    /// packed struct: '<{' (TypeValue ',')* '}>'
    /// vec: '<' (TypeValue ',')* '>'
    /// ```
    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let (tok0, span0) = parser.peek0()?;
        match tok0 {
            FinalToken::LBrace => {
                parser.advance_n(1)?;
                Self::parse_values(parser, AggrKind::Struct, &[FinalToken::RBrace])
                    .map_err(|e| e.map_span(|span| span0.start..span.end))
            }
            FinalToken::LBracket => {
                parser.advance_n(1)?;
                Self::parse_values(parser, AggrKind::Array, &[FinalToken::RBracket])
                    .map_err(|e| e.map_span(|span| span0.start..span.end))
            }
            FinalToken::LAngle => Self::parse_angled(parser),
            tok => {
                parse_err!(Unmatch span0, "aggregates require '<...>' '<{{ ... }}>' '{{ ... }}' '[ ... ]' quote but got token {tok:?}")
            }
        }
    }
}
impl Aggr {
    fn parse_angled(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let start_pos = parser.advance_exact(&[FinalToken::LAngle])?.start;
        let (tok1, _) = parser.peek0()?;
        if tok1 == FinalToken::LBrace {
            // starts with '<{': branch 'packed struct'
            parser.advance_n(1)?;
            Self::parse_values(
                parser,
                AggrKind::PackStruct,
                &[FinalToken::RBrace, FinalToken::RAngle],
            )
            .map_err(|e| e.map_span(|span| start_pos..span.end))
        } else {
            // starts with '<': branch 'fix vector'
            Self::parse_values(parser, AggrKind::Vec, &[FinalToken::RAngle])
                .map_err(|e| e.map_span(|span| start_pos..span.end))
        }
    }

    fn parse_values(
        parser: &mut IRParser<'_>,
        kind: AggrKind,
        ending: &[FinalToken],
    ) -> IRParseRes<Self> {
        let mut elems: Vec<TypeValue> = Vec::new();
        let begin_pos = parser.parser_pos();
        loop {
            let (tok, _) = parser.peek0()?;
            if tok.eq(&ending[0]) {
                break;
            }
            let tv = TypeValue::parse(parser)?;
            elems.push(tv);
            if let (FinalToken::Comma, _) = parser.peek0()? {
                parser.advance_n(1)?;
            } else {
                break;
            }
        }
        let end_pos = parser.advance_exact(ending)?.end;
        Ok(Self {
            span: begin_pos..end_pos,
            kind,
            elems,
        })
    }
}

/// Sparse key-value array expression.
#[derive(Debug, Clone)]
pub struct SparseExpr {
    pub span: logos::Span,
    pub elems: Vec<(usize, TypeValue)>,
    pub default: Box<TypeValue>,
}
impl From<SparseExpr> for Operand {
    fn from(value: SparseExpr) -> Self {
        let span = value.get_span();
        Operand {
            span,
            kind: OperandKind::Sparse(value),
        }
    }
}
impl AstNode for SparseExpr {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser
            .advance_exact(&[FinalToken::lit_word("sparse"), FinalToken::LBracket])?
            .start;
        let mut elems = Vec::new();
        // body: `[Index] = Type Value,`
        loop {
            if let (FinalToken::DotDotEq, _) = parser.peek0()? {
                break;
            }
            parser
                .advance_exact(&[FinalToken::LBracket])
                .map_err(|e| e.map_span(|s| begin_pos..s.end))?;
            let index = match parser.peek0()? {
                (FinalToken::LitInt(index), _) => index as usize,
                (_, span) => {
                    return parse_err!(Unmatch begin_pos..span.end, "sparse array expr element is `[ index ] = ty value,`");
                }
            };
            parser.advance_n(1)?;
            parser
                .advance_exact(&[FinalToken::RBracket, FinalToken::Eq])
                .map_err(|e| e.map_span(|s| begin_pos..s.end))?;
            let tv = TypeValue::parse(parser)?;
            elems.push((index, tv));
            parser
                .advance_exact(&[FinalToken::Comma])
                .map_err(|e| e.map_span(|s| begin_pos..s.end))?;
        }
        parser.advance_exact(&[FinalToken::DotDotEq])?;
        let default_tv =
            TypeValue::parse(parser).map_err(|e| e.map_span(|span| begin_pos..span.end))?;
        if let (FinalToken::Comma, _) = parser.peek0()? {
            parser.advance_n(1)?
        }
        let end_pos = parser.advance_exact(&[FinalToken::RBracket])?.end;
        Ok(Self {
            span: begin_pos..end_pos,
            elems,
            default: Box::new(default_tv),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    pub span: logos::Span,
    pub name: SmolStr,
}

impl AstNode for Label {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn repr(&self) -> String {
        let Self { span, name } = self;
        format!("label {name:?} ({span:?})")
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser
            .advance_exact(&[FinalToken::lit_word("label")])?
            .start;
        match parser.peek0()? {
            (FinalToken::PIdent(id), span) => {
                parser.advance_n(1)?;
                Ok(Self {
                    span: begin_pos..span.end,
                    name: id,
                })
            }
            (tok, span) => {
                parse_err!(Unmatch begin_pos..span.end, "label requires 'label %id' but got tokens [Label, {tok:?}]")
            }
        }
    }
}
impl Label {
    pub fn make_ident(&self) -> Ident {
        Ident {
            kind: IdentKind::Local,
            name: self.name.clone(),
            span: self.get_span(),
        }
    }
}
