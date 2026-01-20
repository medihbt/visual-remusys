use smol_str::SmolStr;
use std::fmt::Debug;

use crate::{
    parse_err,
    parser::{IRParseErr, IRParseErrKind, IRParseRes, IRParser},
    tokens::FinalToken,
};

pub trait AstNode: Debug {
    fn get_span(&self) -> logos::Span;

    fn repr(&self) -> String {
        format!("{self:#?}")
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self>
    where
        Self: Sized;

    fn try_parse(parser: &mut IRParser<'_>) -> IRParseRes<Option<Self>>
    where
        Self: Sized,
    {
        let index = parser.get_token_index();
        match Self::parse(parser) {
            Ok(ret) => Ok(Some(ret)),
            Err(IRParseErr { kind, span }) => match kind {
                IRParseErrKind::Unmatch(_) => {
                    parser.set_token_index(index);
                    Ok(None)
                }
                kind => Err(IRParseErr { kind, span }),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentKind {
    /// Syntax: `Word`
    Word,
    /// Syntax: `%Word`
    Local,
    /// Syntax: `@Word`
    Global,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Ident {
    pub kind: IdentKind,
    pub name: SmolStr,
    pub span: logos::Span,
}

impl Debug for Ident {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { kind, name, span } = self;
        write!(f, "Ident::{kind:?} ({span:?}: {name:?})")
    }
}

impl AstNode for Ident {
    fn get_span(&self) -> logos::Span {
        self.span.clone()
    }

    fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
        let (tok, span) = parser.peek0()?;
        let (kind, name) = match tok {
            FinalToken::Word(word) => (IdentKind::Word, word),
            FinalToken::AIdent(id) => (IdentKind::Global, id),
            FinalToken::PIdent(id) => (IdentKind::Local, id),
            _ => return parse_err!(Unmatch span, "requires word | @word | %word for identifiers"),
        };
        parser.advance_n(1)?;
        Ok(Self { kind, name, span })
    }
}

pub use typing::*;
mod typing {
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
        fn parse_word(
            parser: &mut IRParser<'_>,
            word: &str,
            span: logos::Span,
        ) -> IRParseRes<Self> {
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
                    return parse_err!(
                        Unmatch begin_span.start..next_span.end,
                        "expected angled type '< N x T >' or '<{{ T, T, ... }}>' but got token {next:?}"
                    );
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
}

pub use value::*;
mod value {
    use std::sync::Arc;

    use crate::{
        ast::{AstNode, TypeAst},
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
    #[derive(Debug, Clone)]
    pub struct Operand {
        pub span: logos::Span,
        pub kind: OperandKind,
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

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

pub use inst::*;
mod inst;

#[cfg(test)]
mod tests {
    use super::*;

    fn ast_test_common<T: AstNode>(inputs: &[&str]) {
        struct ParserErr<'a>(&'a IRParser<'a>, IRParseErr);

        impl<'a> Debug for ParserErr<'a> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let Self(parser, err) = self;
                parser.print_fmt_err(err, f)
            }
        }

        for &input in inputs {
            let mut parser = IRParser::new(input);
            match T::parse(&mut parser) {
                Ok(t) => println!("parse({input:?}) => {t:#?}"),
                Err(err) => {
                    panic!("{:?}", ParserErr(&parser, err))
                }
            }
        }
    }

    #[test]
    fn test_type_parsing() {
        ast_test_common::<TypeAst>(&[
            "i1",
            "i8",
            "i32",
            "i64",
            "i128",
            "float",
            "[ 3 x float ]",
            "{ i8, [ 3 x i8 ], i32 }",
            "[ 30 x ptr ]",
            "[ 10 x %MyStruct ]",
        ]);
    }

    #[test]
    fn test_value_parsing() {
        ast_test_common::<Operand>(&[
            "10",
            "false",
            "true",
            "%local",
            "@global",
            "[ i32 1, i32 2, i32 3 ]",
            "{ i32 10, [4 x i8] zeroinitialzier, i64 20 }",
            r#"c"Hello, world""#,
        ]);
    }

    #[test]
    fn test_inst_parsing() {
        ast_test_common::<InstAst>(&[
            "unreachable",
            "%init = alloca i32",
            "%val = load i32, ptr %init, align 4",
            r#"switch i32 %val, label %default, [
                i32 0, label %case0
                i32 1, label %case1
                i32 2, label %case2
            ]"#
        ]);
    }
}
