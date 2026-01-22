use smol_str::SmolStr;
use std::fmt::Debug;

use crate::{
    parse_err,
    parser::{IRParseErr, IRParseErrKind, IRParseRes, IRParser},
    tokens::FinalToken,
};

mod inst;
mod typing;
mod value;

pub use self::{inst::*, typing::*, value::*};

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

pub use cfg::*;
mod cfg {
    use std::fmt::Debug;

    use smol_str::{SmolStr, format_smolstr};

    use crate::{
        ast::{AstNode, Ident, IdentKind, InstAst},
        parser::{IRParseErrKind, IRParseRes, IRParser},
        tokens::FinalToken,
    };

    pub struct BlockAst {
        pub label: Ident,
        pub insts: Vec<InstAst>,
    }
    impl Debug for BlockAst {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { label, insts } = self;
            write!(f, "BlockAst (label: {:?}) ", label)?;
            f.debug_list().entries(insts.iter()).finish()
        }
    }
    impl AstNode for BlockAst {
        fn get_span(&self) -> logos::Span {
            let mut span = self.label.get_span();
            if let Some(end) = self.insts.last() {
                span.end = end.get_span().end;
            }
            span
        }

        fn parse(_: &mut IRParser<'_>) -> IRParseRes<Self> {
            panic!("Not supported: BlockAst::parse");
        }
    }

    #[derive(Debug)]
    pub enum FuncLine {
        Label(Ident),
        Inst(Box<InstAst>),
    }
    impl AstNode for FuncLine {
        fn get_span(&self) -> logos::Span {
            match self {
                Self::Label(l) => l.get_span(),
                Self::Inst(i) => i.get_span(),
            }
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            fn parse_inst(parser: &mut IRParser<'_>) -> IRParseRes<FuncLine> {
                let inst = InstAst::parse(parser)?;
                Ok(FuncLine::Inst(Box::new(inst)))
            }
            fn parse_label(
                parser: &mut IRParser<'_>,
                span0: logos::Span,
                label_name: SmolStr,
            ) -> IRParseRes<FuncLine> {
                let span1 = match parser.peek1() {
                    Ok((FinalToken::Colon, span)) => span,
                    Ok(..) => return parse_inst(parser),
                    Err(e) if e.kind == IRParseErrKind::EndOfInput => {
                        return parse_inst(parser);
                    }
                    Err(e) => return Err(e),
                };
                parser.advance_n(2)?;
                Ok(FuncLine::Label(Ident {
                    kind: IdentKind::Local,
                    name: label_name,
                    span: span0.start..span1.end,
                }))
            }

            let (tok0, span0) = parser.peek0()?;
            match tok0 {
                FinalToken::Word(label_name) => parse_label(parser, span0, label_name),
                FinalToken::LitInt(id) => parse_label(parser, span0, format_smolstr!("{id}")),
                _ => parse_inst(parser),
            }
        }
    }

    pub struct FuncBodyAst {
        pub span: logos::Span,
        pub blocks: Vec<BlockAst>,
    }
    impl Debug for FuncBodyAst {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { span, blocks } = self;
            write!(f, "FuncBodyAst (span: {span:?}) ")?;
            f.debug_list().entries(blocks.iter()).finish()
        }
    }
    impl AstNode for FuncBodyAst {
        fn get_span(&self) -> logos::Span {
            self.span.clone()
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            let begin_pos = parser.advance_exact(&[FinalToken::LBrace])?.start;
            let mut func_body = FuncBodyAst {
                span: begin_pos..begin_pos,
                blocks: Vec::new(),
            };
            loop {
                if parser.peek0_match(FinalToken::RBrace)?.0 {
                    break;
                }
                func_body.add_line(FuncLine::parse(parser)?);
            }
            let end_pos = parser.advance_exact(&[FinalToken::RBrace])?.end;
            func_body.span.end = end_pos;
            Ok(func_body)
        }
    }
    impl FuncBodyAst {
        fn add_line(&mut self, line: FuncLine) {
            match line {
                FuncLine::Label(label) => self.blocks.push(BlockAst {
                    label,
                    insts: Vec::new(),
                }),
                FuncLine::Inst(inst) if self.blocks.is_empty() => {
                    let span = inst.get_span();
                    self.blocks.push(BlockAst {
                        label: Ident {
                            kind: IdentKind::Word,
                            name: SmolStr::new(""),
                            span: span.start..span.start,
                        },
                        insts: vec![*inst],
                    });
                }
                FuncLine::Inst(inst) => {
                    let last_block = self.blocks.last_mut().unwrap();
                    last_block.insts.push(*inst);
                }
            }
        }
    }
}

pub use module::*;
mod module {
    use std::{fmt::Debug, sync::Arc};

    use remusys_ir::ir::{Linkage, TLSModel};
    use smol_str::SmolStr;

    use crate::{
        ast::{AstNode, FuncBodyAst, Ident, IdentKind, Operand, TypeAst, utils},
        parse_err,
        parser::{IRParseErrKind, IRParseRes, IRParser},
        tokens::FinalToken,
    };

    #[derive(Debug)]
    pub struct FuncArg {
        pub ty: TypeAst,
        pub name: Ident,
    }
    impl AstNode for FuncArg {
        fn get_span(&self) -> logos::Span {
            self.ty.get_span().start..self.name.get_span().end
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            let ty = TypeAst::parse(parser)?;
            match parser.peek0()? {
                (FinalToken::Comma, _) | (FinalToken::RParen, _) => Ok(Self::new_unnamed(ty)),
                (FinalToken::PIdent(name), span) => {
                    let kind = IdentKind::Local;
                    let name = Ident { span, name, kind };
                    parser.advance_n(1)?;
                    Ok(Self { ty, name })
                }
                (FinalToken::Word(attr), span) => {
                    parse_err!(Unmatch ty.span.start..span.end, "this parser has not supported attrs yet (maybe attr {attr:?})")
                }
                (tok, span) => {
                    parse_err!(Unmatch ty.span.start..span.end, "invalid FuncArg token {tok:?}")
                }
            }
        }
    }
    impl FuncArg {
        fn new_unnamed(ty: TypeAst) -> Self {
            let name = Ident {
                span: ty.span.end..ty.span.end,
                kind: IdentKind::Local,
                name: SmolStr::new_inline(""),
            };
            Self { ty, name }
        }
    }

    /// function header
    ///
    /// Syntax:
    ///
    /// ```remusys_ir
    /// declare [Linkage]? <ret_ty> @name ParamList
    /// define [Linkage]? <ret_ty> @name ParamList
    ///
    /// ParamList: '(' (ParamUnit,)* ')'
    /// Linkage: 'private' | 'internal' | 'external' | 'dso_local'
    /// ```
    #[derive(Debug)]
    pub struct FuncHeader {
        pub span: logos::Span,
        pub is_declare: bool,
        pub linkage: Linkage,
        pub name: SmolStr,
        pub ret_ty: TypeAst,
        pub args: Vec<FuncArg>,
    }
    impl AstNode for FuncHeader {
        fn get_span(&self) -> logos::Span {
            self.span.clone()
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            let (is_declare, begin_pos) = Self::parse_head(parser)?;
            let linkage = {
                let linkage = if is_declare {
                    Linkage::External
                } else {
                    Linkage::Private
                };
                utils::parse_linkage(parser)?.unwrap_or(linkage)
            };
            let ret_ty = TypeAst::parse(parser)?;
            let (tok, span) = parser.peek0()?;
            let name = match tok {
                FinalToken::AIdent(id) => {
                    parser.advance_n(1)?;
                    id
                }
                tok => {
                    return parse_err!(Unmatch span, "function name requires '@word' but got token {tok:?}");
                }
            };

            let mut args = Vec::new();
            parser.advance_exact(&[FinalToken::LParen])?;
            loop {
                if parser.peek0_match(FinalToken::RParen)?.0 {
                    break;
                }
                let arg = FuncArg::parse(parser)?;
                args.push(arg);
                if parser.peek0_match(FinalToken::Comma)?.0 {
                    parser.advance_n(1)?;
                }
            }
            let end_pos = parser.advance_exact(&[FinalToken::RParen])?.end;
            Ok(Self {
                span: begin_pos..end_pos,
                is_declare,
                linkage,
                name,
                ret_ty,
                args,
            })
        }
    }
    impl FuncHeader {
        fn parse_head(parser: &mut IRParser<'_>) -> IRParseRes<(bool, usize)> {
            let (word, span) = match parser.peek0()? {
                (FinalToken::Word(word), span) => (word, span),
                (tok, span) => {
                    return parse_err!(
                        Unmatch span,
                        "function header requires 'declare' | 'define' but got token {tok:?}"
                    );
                }
            };
            let is_declare = match word.as_str() {
                "declare" => true,
                "define" => false,
                word => {
                    return parse_err!(
                        Unmatch span,
                        "function header requires 'declare' | 'define' but got word {word:?}"
                    );
                }
            };
            parser.advance_n(1)?;
            Ok((is_declare, span.start))
        }
    }

    /// Syntax:
    ///
    /// ```remusys_ir
    /// FuncAst: FuncHeader FuncBodyAst?
    /// ```
    pub struct FuncAst {
        pub header: FuncHeader,
        pub body: Option<FuncBodyAst>,
    }
    impl Debug for FuncAst {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self { header, body } = self;
            let mut ds = f.debug_struct("FuncAst");
            ds.field("header", header);
            if let Some(body) = body {
                ds.field("body", body);
            }
            ds.finish()
        }
    }
    impl AstNode for FuncAst {
        fn get_span(&self) -> logos::Span {
            let header_span = self.header.get_span();
            match &self.body {
                None => header_span,
                Some(body) => header_span.start..body.get_span().end,
            }
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            let header = FuncHeader::parse(parser)?;
            let body = if header.is_declare {
                None
            } else {
                Some(FuncBodyAst::parse(parser)?)
            };
            Ok(Self { header, body })
        }
    }

    /// Syntax:
    ///
    /// ```remusys_ir
    /// @name = external (global|constant) <type>[, align <align>]?
    /// @name = [Linkage]? (global|constant) <type> <value>[, align <align>]?
    /// ```
    #[derive(Debug)]
    pub struct GlobalVarAst {
        pub span: logos::Span,
        pub name: SmolStr,
        pub linkage: Linkage,
        pub tls_model: Option<TLSModel>,
        /// from syntax: `global => false; constant => true`
        pub is_const: bool,
        pub ty: TypeAst,
        pub init: Option<Operand>,
        pub align: Option<usize>,
    }
    impl AstNode for GlobalVarAst {
        fn get_span(&self) -> logos::Span {
            self.span.clone()
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            use crate::ast::FinalToken as T;
            let (name, begin_pos) = match parser.peek0()? {
                (T::AIdent(id), span) => {
                    parser.advance_n(1)?;
                    (id, span.start)
                }
                (tok, span) => {
                    return parse_err!(Unmatch span, "global variable requires '@word' but got token {tok:?}");
                }
            };
            parser.advance_exact(&[T::Eq])?;
            let linkage = utils::parse_linkage(parser)?.unwrap_or(Linkage::Private);

            let tls_model = utils::parse_tls_model(parser)?;

            let is_const = match parser.peek0()? {
                (T::Word(word), span) => match word.as_str() {
                    "global" => false,
                    "constant" => true,
                    word => {
                        return parse_err!(Unmatch span, "global variable requires 'global' | 'constant' but got word {word:?}");
                    }
                },
                (tok, span) => {
                    return parse_err!(Unmatch span, "global variable requires 'global' | 'constant' but got token {tok:?}");
                }
            };
            parser.advance_n(1)?;

            let ty = TypeAst::parse(parser)?;
            let init = if linkage == Linkage::External {
                None
            } else {
                Some(Operand::parse(parser)?)
            };
            let align = utils::parse_align(parser)?;
            let end_pos = parser.parser_pos();
            Ok(Self {
                span: begin_pos..end_pos,
                name,
                linkage,
                tls_model,
                is_const,
                ty,
                init,
                align,
            })
        }
    }

    /// Syntax:
    ///
    /// ```remusys-ir
    /// %name = type <struct_type>
    ///
    /// <struct_type> is limited to struct type or packed struct type.
    /// this will be verified in semantic analysis phase.
    /// ```
    #[derive(Debug)]
    pub struct TypeAliasItem {
        pub span: logos::Span,
        pub name: SmolStr,
        /// will be checked in semantic analysis phase
        pub ty: TypeAst,
    }
    impl AstNode for TypeAliasItem {
        fn get_span(&self) -> logos::Span {
            self.span.clone()
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            let (name, begin_pos) = match parser.peek0()? {
                (FinalToken::PIdent(id), span) => {
                    parser.advance_n(1)?;
                    (id, span.start)
                }
                (tok, span) => {
                    return parse_err!(Unmatch span, "type alias requires '%word' but got token {tok:?}");
                }
            };
            parser.advance_exact(&[FinalToken::Eq, FinalToken::lit_word("type")])?;
            let ty = TypeAst::parse(parser)?;
            let end_pos = parser.parser_pos();
            Ok(Self {
                span: begin_pos..end_pos,
                name,
                ty,
            })
        }
    }

    #[derive(Debug)]
    pub enum ModuleItem {
        Func(FuncAst),
        GlobalVar(GlobalVarAst),
        TypeAlias(TypeAliasItem),
        Finish(usize),
    }
    impl AstNode for ModuleItem {
        fn get_span(&self) -> logos::Span {
            match self {
                Self::Func(f) => f.get_span(),
                Self::GlobalVar(g) => g.get_span(),
                Self::TypeAlias(t) => t.get_span(),
                Self::Finish(pos) => *pos..*pos,
            }
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            let (tok0, span0) = match parser.peek0() {
                Ok(v) => v,
                Err(e) if e.kind == IRParseErrKind::EndOfInput => {
                    let pos = parser.parser_pos();
                    return Ok(Self::Finish(pos));
                }
                Err(e) => return Err(e),
            };

            match tok0 {
                FinalToken::Word(word) => match word.as_str() {
                    "declare" | "define" => {
                        let func = FuncAst::parse(parser)?;
                        Ok(Self::Func(func))
                    }
                    word => {
                        parse_err!(Unmatch span0, "unexpected word at module level: {word:?}")
                    }
                },
                FinalToken::AIdent(_) => {
                    let glob = GlobalVarAst::parse(parser)?;
                    Ok(Self::GlobalVar(glob))
                }
                FinalToken::PIdent(_) => {
                    let type_alias = TypeAliasItem::parse(parser)?;
                    Ok(Self::TypeAlias(type_alias))
                }
                _ => {
                    parse_err!(Unmatch span0, "unexpected token at module level: {tok0:?}")
                }
            }
        }
    }

    #[derive(Debug)]
    pub struct ModuleAst {
        pub funcs: Vec<FuncAst>,
        pub global_vars: Vec<GlobalVarAst>,
        pub type_aliases: Vec<Arc<TypeAliasItem>>,
        pub span: logos::Span,
    }
    impl AstNode for ModuleAst {
        fn get_span(&self) -> logos::Span {
            self.span.clone()
        }

        fn parse(parser: &mut IRParser<'_>) -> IRParseRes<Self> {
            let begin_pos = parser.parser_pos();
            let mut module = ModuleAst {
                funcs: Vec::new(),
                global_vars: Vec::new(),
                type_aliases: Vec::new(),
                span: begin_pos..begin_pos,
            };
            let end_pos = loop {
                let item = ModuleItem::parse(parser)?;
                match item {
                    ModuleItem::Func(f) => module.funcs.push(f),
                    ModuleItem::GlobalVar(g) => module.global_vars.push(g),
                    ModuleItem::TypeAlias(t) => module.type_aliases.push(Arc::new(t)),
                    ModuleItem::Finish(pos) => break pos,
                }
            };
            module.span.end = end_pos;
            Ok(module)
        }
    }
}

mod utils {
    use super::*;
    use FinalToken as T;
    use remusys_ir::ir::{Linkage, TLSModel};

    pub fn parse_linkage(parser: &mut IRParser<'_>) -> IRParseRes<Option<Linkage>> {
        let (T::Word(word), _) = parser.peek0()? else {
            return Ok(None);
        };
        let linkage = match word.as_str() {
            "private" => Linkage::Private,
            "internal" => Linkage::Private,
            "external" => Linkage::External,
            "dso_local" => Linkage::DSOLocal,
            _ => return Ok(None),
        };
        parser.advance_n(1)?;
        Ok(Some(linkage))
    }
    /// Parse optional tls_model clause.
    pub fn parse_tls_model(parser: &mut IRParser<'_>) -> IRParseRes<Option<TLSModel>> {
        let (T::Word(word), begin_span) = parser.peek0()? else {
            return Ok(None);
        };
        if word != "thread_local" {
            return Ok(None);
        }
        parser.advance_n(1)?;
        parser.advance_exact(&[T::LParen])?;

        let (model_name, model_span) = match parser.peek0()? {
            (T::Word(model_word), span) => (model_word, span),
            (tok, span) => {
                return parse_err!(Unmatch span, "tls_model requires model word but got token {tok:?}");
            }
        };
        let model = match TLSModel::from_ir_text(&model_name) {
            Some(model) => model,
            None => {
                let span = begin_span.start..model_span.end;
                return parse_err!(
                    Unmatch span,
                    "unknown tls_model name: '{model_name}', supported models are: \
                    'generaldynamic', 'localdynamic', 'initialexec', 'localexec'"
                );
            }
        };
        parser.advance_n(1)?;
        parser.advance_exact(&[T::RParen])?;
        Ok(Some(model))
    }

    /// Parse optional align clause.
    ///
    /// Syntax: `, align <align>`
    pub fn parse_align(parser: &mut IRParser<'_>) -> IRParseRes<Option<usize>> {
        let span = match parser.peek0() {
            Ok((T::Comma, span)) => span,
            Ok((..)) => return Ok(None),
            Err(e) if e.kind == IRParseErrKind::EndOfInput => return Ok(None),
            Err(e) => return Err(e),
        };
        parser.advance_exact(&[T::Comma, T::lit_word("align")])?;
        let res = match parser.peek0()? {
            (T::LitInt(i), _) => Some(i as usize),
            _ => {
                let span = span.start..parser.parser_pos();
                return parse_err!(Unmatch span, "align requires literal int");
            }
        };
        parser.advance_n(1)?;
        Ok(res)
    }
}
#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    struct ParserErr<'a>(&'a IRParser<'a>, IRParseErr);

    impl<'a> Debug for ParserErr<'a> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Self(parser, err) = self;
            parser.print_fmt_err(err, f)
        }
    }

    fn ast_test_common<T: AstNode>(inputs: &[&str]) {
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
            "sparse [ [0] = i32 10, [2] = i32 20, ..= i32 0 ]",
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
            ]"#,
            r"%res = call i32 (...) @printf(ptr %fmt, i32 1, i32 2)",
        ]);
    }

    #[test]
    fn test_parsing() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join("main.ll");
        let input = std::fs::read_to_string(dir).unwrap();
        let mut parser = IRParser::new(&input);
        match ModuleAst::parse(&mut parser) {
            Ok(module) => {
                println!("Parsed module:\n{:#?}", module);
            }
            Err(err) => {
                panic!("{:?}", ParserErr(&parser, err));
            }
        }
    }
}
