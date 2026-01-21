use std::ops::Range;

use super::*;
use remusys_ir::typing::FPKind;

#[derive(Debug, Clone)]
pub enum TypeAstKind {
    /// Syntax: `'void'`
    Void,
    /// Syntax: `'ptr'`
    Ptr,
    /// Syntax: `'i1' | 'i8' | 'i16' | 'i32' | 'i64' | 'i128'`
    Int(u8),
    /// Syntax: `'float' | 'double'`
    FP(FPKind),
    /// Syntax: `'[' T_INT 'x' TypeAst ']'`
    Array { elem: Box<TypeAst>, len: usize },
    /// Syntax: `'<' T_INT 'x' TypeAst '>'`
    Vec { elem: Box<TypeAst>, len: usize },
    /// Syntax:
    ///
    /// ```remusys_ir
    /// packed: '<' '{' (TypeAst ',')* '}' '>'
    /// normal: '{' (TypeAst ',')* '}'
    /// ```
    Struct { elem: Box<[TypeAst]>, packed: bool },
    /// Syntax: `%word`
    Alias(Ident),
}

#[derive(Clone)]
pub struct TypeAst {
    pub span: logos::Span,
    pub kind: TypeAstKind,
}

impl Debug for TypeAst {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { span, kind } = self;
        match kind {
            TypeAstKind::Void => write!(f, "TypeAst::Void ({span:?})"),
            TypeAstKind::Ptr => write!(f, "TypeAst::Ptr ({span:?})"),
            TypeAstKind::Int(bits) => write!(f, "TypeAst::i{bits} ({span:?})"),
            TypeAstKind::FP(fpkind) => write!(f, "TypeAst::{fpkind:?} ({span:?})"),
            TypeAstKind::Array { elem, len } => f
                .debug_struct("TypeAst::Array")
                .field("span", span)
                .field("elem", elem)
                .field("len", len)
                .finish(),
            TypeAstKind::Vec { elem, len } => f
                .debug_struct("TypeAst::Vec")
                .field("span", span)
                .field("elem", elem)
                .field("len", len)
                .finish(),
            TypeAstKind::Struct { elem, packed } => f
                .debug_struct("TypeAst::Struct")
                .field("span", span)
                .field("packed", packed)
                .field("elem", elem)
                .finish(),
            TypeAstKind::Alias(ident) => {
                write!(f, "TypeAst::Alias ({span:?} name {:?})", &ident.name)
            }
        }
    }
}

impl AstNode for TypeAst {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let (tok, span) = parser.peek0()?;
        match tok {
            FinalToken::Word(word) => Self::parse_word(parser, word.as_str(), span),
            FinalToken::PIdent(name) => {
                parser.advance_n(1)?;
                Ok(TypeAst {
                    span: span.clone(),
                    kind: TypeAstKind::Alias(Ident {
                        kind: IdentKind::Local,
                        name,
                        span,
                    }),
                })
            }
            FinalToken::LBracket => Self::parse_array(parser),
            FinalToken::LBrace => Self::parse_struct(parser, false),
            FinalToken::LAngle => Self::parse_angled(parser),
            _ => parse_err!(Unmatch span, "unexpected token for typing AST"),
        }
    }
}

impl TypeAst {
    fn parse_word(parser: &mut IRParser<'_>, word: &str, span: logos::Span) -> IRParseRes<Self> {
        let kind = match word {
            "void" => TypeAstKind::Void,
            "ptr" => TypeAstKind::Ptr,
            "float" => TypeAstKind::FP(FPKind::Ieee32),
            "double" => TypeAstKind::FP(FPKind::Ieee64),
            "i1" => TypeAstKind::Int(1),
            "i8" => TypeAstKind::Int(8),
            "i16" => TypeAstKind::Int(16),
            "i32" => TypeAstKind::Int(32),
            "i64" => TypeAstKind::Int(64),
            "i128" => TypeAstKind::Int(128),
            _ => return parse_err!(Unmatch span, "unexpected word for typing AST"),
        };
        parser.advance_n(1)?;
        Ok(TypeAst { span, kind })
    }

    fn parse_array(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let Range {
            start: start_pos,
            end: _,
        } = parser.advance_exact(&[FinalToken::LBracket])?;

        let length = {
            let (tok, span) = parser.peek0()?;
            let FinalToken::LitInt(length) = &tok else {
                return parse_err!(Unmatch start_pos..span.end, "unexpected length for array type but got {tok:?}");
            };
            parser.advance_n(1)?;
            *length as usize
        };
        parser.advance_exact(&[FinalToken::lit_word("x")])?;

        let elemty = TypeAst::parse(parser)?;
        let Range {
            start: _,
            end: end_pos,
        } = parser.advance_exact(&[FinalToken::RBracket])?;

        Ok(Self {
            span: start_pos..end_pos,
            kind: TypeAstKind::Array {
                elem: Box::new(elemty),
                len: length,
            },
        })
    }

    fn parse_struct(parser: &mut IRParser<'_>, packed: bool) -> IRParseRes<Self> {
        let start_pos = if packed {
            parser
                .advance_exact(&[FinalToken::LAngle, FinalToken::LBrace])?
                .start
        } else {
            parser.advance_exact(&[FinalToken::LBrace])?.start
        };
        let mut elems = Vec::new();
        loop {
            let (matches, _) = parser.peek0_match(FinalToken::RBrace)?;
            if matches {
                break;
            }
            let elemty = TypeAst::parse(parser)?;
            if parser.peek0_match(FinalToken::Comma)?.0 {
                parser.advance_n(1)?;
            }
            elems.push(elemty);
        }
        let end_pos = if packed {
            parser
                .advance_exact(&[FinalToken::RBrace, FinalToken::RAngle])?
                .end
        } else {
            parser.advance_exact(&[FinalToken::RBrace])?.end
        };
        Ok(Self {
            span: start_pos..end_pos,
            kind: TypeAstKind::Struct {
                elem: elems.into_boxed_slice(),
                packed,
            },
        })
    }

    fn parse_angled(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let (FinalToken::LAngle, begin_span) = parser.peek0()? else {
            panic!("should have been verified: starts with '<'");
        };
        let (next, next_span) = parser.peek1()?;
        match next {
            FinalToken::LitInt(_) => Self::parse_fixvec(parser),
            FinalToken::LBrace => Self::parse_struct(parser, true),
            next => {
                parse_err!(
                    Unmatch begin_span.start..next_span.end,
                    "expected angled type '< N x T >' or '<{{ T, T, ... }}>' but got token {next:?}"
                )
            }
        }
    }

    fn parse_fixvec(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let begin_pos = parser.advance_exact(&[FinalToken::LAngle])?.start;
        let (length_tok, span) = parser.peek0()?;
        let FinalToken::LitInt(length) = length_tok else {
            return parse_err!(Unmatch begin_pos..span.end, "expected vec type '< N x T >'");
        };
        parser.advance_n(1)?;
        parser.advance_exact(&[FinalToken::lit_word("x")])?;
        let elemty = TypeAst::parse(parser)?;
        let end_pos = parser.advance_exact(&[FinalToken::RAngle])?.end;

        match &elemty.kind {
            TypeAstKind::Int(_) | TypeAstKind::FP(_) | TypeAstKind::Ptr => {}
            _ => return parse_err!(TypeErrInvalidVecElem begin_pos..end_pos),
        }

        Ok(Self {
            span: begin_pos..end_pos,
            kind: TypeAstKind::Vec {
                elem: Box::new(elemty),
                len: length as usize,
            },
        })
    }
}
