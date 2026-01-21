use std::ops::Range;

use crate::tokens::{FinalToken, IRLexer};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum IRParseErrKind {
    #[error("lexical error: {0}")]
    Lexical(String),

    #[error("cannot match: {0}")]
    Unmatch(String),

    #[error("end of input")]
    EndOfInput,

    #[error("type error: fix vector type element should be int or float")]
    TypeErrInvalidVecElem,

    #[error("inst error: condition operand requires `i1` type")]
    InstErrInvalidCondType,
}

#[derive(Debug)]
pub struct IRParseErr {
    pub kind: IRParseErrKind,
    pub span: logos::Span,
}
pub type IRParseRes<T = ()> = Result<T, IRParseErr>;
type ParseLexRes = IRParseRes<(FinalToken, logos::Span)>;

impl IRParseErr {
    pub fn lexical(err: String, span: logos::Span) -> Self {
        Self {
            kind: IRParseErrKind::Lexical(err),
            span,
        }
    }
    pub fn unmatch(reason: String, span: logos::Span) -> Self {
        Self {
            kind: IRParseErrKind::Unmatch(reason),
            span,
        }
    }
    pub fn endof_input(span: logos::Span) -> Self {
        Self {
            kind: IRParseErrKind::EndOfInput,
            span,
        }
    }

    pub fn map_span(mut self, mut edit: impl FnMut(logos::Span) -> logos::Span) -> Self {
        self.span = edit(self.span);
        self
    }
}

#[macro_export]
macro_rules! parse_err {
    (Lexical $span:expr) => {
        parse_err!(Lexical $span, "")
    };
    (Lexical $span:expr, $($arg:tt)*) => {
        Err($crate::parser::IRParseErr::lexical(format!($($arg)*), $span))
    };
    (Unmatch $span:expr) => {
        parse_err!(Unmatch $span, String::from("unknown"))
    };
    (Unmatch $span:expr, $($arg:tt)*) => {
        Err($crate::parser::IRParseErr::unmatch(format!($($arg)*), $span))
    };
    (EndOfInput $span:expr) => {
        Err($crate::parser::IRParseErr::endof_input($span))
    };
    (TypeErrInvalidVecElem $span:expr) => {
        Err($crate::parser::IRParseErr {
            kind: $crate::parser::IRParseErrKind::TypeErrInvalidVecElem,
            span: $span
        })
    };
    (InstErrInvalidCondType $span:expr) => {
        Err($crate::parser::IRParseErr {
            kind: $crate::parser::IRParseErrKind::InstErrInvalidCondType,
            span: $span
        })
    };
}

pub struct IRParser<'src> {
    lexer: IRLexer<'src>,
    tokens: Vec<(FinalToken, logos::Span)>,
    line_pos: Vec<usize>,
    index: usize,
}

impl<'src> IRParser<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            lexer: IRLexer::new(source),
            tokens: Vec::new(),
            line_pos: Vec::new(),
            index: 0,
        }
    }

    pub fn get_token_index(&self) -> usize {
        self.index
    }
    pub fn set_token_index(&mut self, index: usize) {
        assert!(
            self.index >= index,
            "index {index} goes further than {}",
            self.index
        );
        self.index = index;
    }

    pub fn get_source(&self) -> &'src str {
        self.lexer.get_source()
    }
    pub fn parser_pos(&self) -> usize {
        if let Some((_, span)) = self.tokens.get(self.index) {
            return span.start;
        }
        if self.tokens.is_empty() || self.index == 0 {
            return 0;
        }
        if let Some((_, span)) = self.tokens.get(self.index - 1) {
            return span.end;
        }
        0
    }
    pub fn print_err(
        &self,
        err: &IRParseErr,
        write: &mut dyn std::io::Write,
    ) -> std::io::Result<()> {
        let IRParseErr {
            kind,
            span: Range { start, end },
        } = err;
        let line = self.line_pos.binary_search(start).unwrap_or_else(|x| x);
        writeln!(write, "========== [begin parser error] ==========")?;
        writeln!(write, "at source pos {start}..{end} (line {line}): {kind}")?;
        writeln!(write, "{}", &self.get_source()[*start..*end])?;
        writeln!(write, "=========== [end parser error] ===========")
    }
    pub fn print_fmt_err(
        &self,
        err: &IRParseErr,
        write: &mut dyn std::fmt::Write,
    ) -> std::fmt::Result {
        let IRParseErr {
            kind,
            span: Range { start, end },
        } = err;
        let line = self.line_pos.binary_search(start).unwrap_or_else(|x| x);
        writeln!(write, "========== [begin parser error] ==========")?;
        writeln!(write, "at source pos {start}..{end} (line {line}): {kind}")?;
        writeln!(write, "{}", &self.get_source()[*start..*end])?;
        writeln!(write, "=========== [end parser error] ===========")
    }

    pub fn peek_n(&mut self, n: usize) -> ParseLexRes {
        let index = self.index + n;
        let begin_pos = self.lexer.get_pos();
        let source_len = self.get_source().len();
        while self.tokens.len() <= index {
            let Some((res, span)) = self.lexer.next() else {
                return parse_err!(EndOfInput begin_pos..source_len);
            };
            let token = res.map_err(|err| IRParseErr::lexical(err, span.clone()))?;
            self.tokens.push((token, span));
        }
        let end_pos = self.lexer.get_pos();
        for pos in begin_pos..end_pos {
            let byte = self.get_source().as_bytes()[pos];
            if byte == b'\n' {
                self.line_pos.push(pos);
            }
        }
        Ok(self.tokens[index].clone())
    }
    pub fn peek0(&mut self) -> ParseLexRes {
        self.peek_n(0)
    }
    pub fn peek0_match(&mut self, pat: FinalToken) -> IRParseRes<(bool, logos::Span)> {
        let (tok, span) = self.peek0()?;
        Ok((tok.eq(&pat), span))
    }
    pub fn peek1(&mut self) -> ParseLexRes {
        self.peek_n(1)
    }
    pub fn peek2(&mut self) -> ParseLexRes {
        self.peek_n(2)
    }

    pub fn advance_n(&mut self, n: usize) -> IRParseRes {
        if n > 0 {
            self.peek_n(n - 1)?;
        }
        self.index += n;
        Ok(())
    }

    /// Advance the parser if the next tokens exactly match the given kinds.
    /// Returns an error if any token does not match.
    pub fn advance_exact(&mut self, kinds: &[FinalToken]) -> IRParseRes<logos::Span> {
        let begin_pos = self.parser_pos();
        for kind in kinds {
            let (token, span) = self.peek0()?;
            if kind.eq(&token) {
                self.advance_n(1)?;
                continue;
            }
            return parse_err!(Unmatch span, "expected token {kind:?} but got token {token:?}");
        }
        let end_pos = self.parser_pos();
        Ok(begin_pos..end_pos)
    }
}
