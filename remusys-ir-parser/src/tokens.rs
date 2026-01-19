use logos::Logos;
use smallvec::SmallVec;
use smol_str::SmolStr;

#[derive(Debug, Clone, Logos)]
enum PrimToken<'lex> {
    #[regex(r"[+-]?[0-9]+", |lex| PrimToken::parse_dec_int(lex.slice()))]
    LitDecInt(i128),

    #[regex(r"[+-]?(0X|0x)[0-9A-Fa-f]+", |lex| PrimToken::parse_hex_int(lex.slice()))]
    LitHexInt(i128),

    #[regex(r#"c\"([^\\\"]|(\\[0-9A-Fa-f][0-9A-Fa-f]))*\""#)]
    LitBytes(&'lex str),

    #[regex(r"[+-]?[0-9]*\.[0-9]+", PrimToken::parse_dec_fp)]
    LitFPDec0Std(f64),

    #[regex(r"[+-]?[0-9]+\.", PrimToken::parse_dec_fp)]
    LitFPDec1Std(f64),

    #[regex(r"[+-]?[0-9]*\.[0-9]+[eE][+-]?[0-9]+", PrimToken::parse_dec_fp)]
    LitFPDec0Exp(f64),

    #[regex(r"[+-]?[0-9]+(\.)?[eE][+-]?[0-9]+", PrimToken::parse_dec_fp)]
    LitFPDec1Exp(f64),

    #[regex(r"[+-]?0[xX][0-9A-Fa-f]*\.[0-9A-Fa-f]+[pP][+-]?[0-9]+")]
    LitFPHex0(&'lex str),

    #[regex(r"[+-]?0[xX][0-9A-Fa-f]+(\.)?[pP][+-]?[0-9]+")]
    LitFPHex1(&'lex str),

    #[regex(r"%[a-z0-9A-Z_]+", |lex| &lex.slice()[1..])]
    PIdent(&'lex str),

    #[regex(r"@[a-z0-9A-Z_]+", |lex| &lex.slice()[1..])]
    AIdent(&'lex str),

    #[regex(r"[a-zA-Z_][a-z0-9A-Z_]*")]
    Word(&'lex str),

    #[regex(r":")]
    Semi,
    #[regex(r",")]
    Comma,
    #[regex(r"!")]
    Exclaim,
    #[regex(r"..=")]
    DotDotEq,
    #[regex(r"=")]
    Eq,

    #[regex(r"\(")]
    LParen,
    #[regex(r"\)")]
    RParen,

    #[regex(r"\[")]
    LBracket,
    #[regex(r"\]")]
    RBracket,

    #[regex(r"\{")]
    LBrace,
    #[regex(r"\}")]
    RBrace,

    #[regex(r"<")]
    LAngle,
    #[regex(r">")]
    RAngle,

    #[regex(r"[ \n\r\t\f]+", logos::skip)]
    Space,

    #[regex(r";[^\n\r]*", logos::skip, allow_greedy = true)]
    LineComment,
}

impl<'lex> PrimToken<'lex> {
    fn parse_dec_int(s: &str) -> i128 {
        i128::from_str_radix(s, 10).expect("invalid dec int")
    }
    fn parse_hex_int(input: &str) -> i128 {
        if input.is_empty() {
            return 0;
        }
        let (s, sign) = match input.as_bytes()[0] {
            b'+' => (&input[1..], 1i128),
            b'-' => (&input[1..], -1i128),
            _ => (input, 1i128),
        };
        let src = if s.starts_with("0x") || s.starts_with("0X") {
            &s[2..]
        } else {
            unreachable!("Hex int should start with '0x' or '0X' but got {input}")
        };
        let i = i128::from_str_radix(src, 16).expect("invalid hex int");
        i * sign
    }
    fn parse_bytes(s: &str) -> Result<SmallVec<[u8; 16]>, &'static str> {
        let mut bytes: SmallVec<[u8; 16]> = SmallVec::with_capacity(s.len() - 3);
        debug_assert!(
            s.starts_with("c\""),
            "Byte literal should start with 'c\"' but got {s}"
        );
        debug_assert!(
            s.ends_with('"'),
            "Byte literal should end eith '\"' but got {s}"
        );
        let mut chars = s.chars().skip(2);
        while let Some(c) = chars.next() {
            match c {
                '"' => break,
                '\\' => {
                    let c1 = chars.next().expect("expects 2 hex numbers");
                    let c2 = chars.next().expect("expects 2 hex numbers");
                    let d1 = c1.to_digit(16).unwrap() as u8;
                    let d2 = c2.to_digit(16).unwrap() as u8;
                    bytes.push(d1 * 16 + d2);
                }
                c if c.is_ascii() => {
                    bytes.push(c as u8);
                }
                _ => return Err("Byte string only accepts ASCII bytes"),
            }
        }
        Ok(bytes)
    }
    fn parse_dec_fp(lex: &logos::Lexer<'lex, Self>) -> f64 {
        lex.slice().parse::<f64>().unwrap()
    }
    fn parse_hex_fp(lex: &str) -> Result<f64, String> {
        hexf_parse::parse_hexf64(lex, false).map_err(|e| e.to_string())
    }

    pub fn to_final(self) -> Result<FinalToken, String> {
        use PrimToken::*;
        let res = match self.clone() {
            Word(s) => match Self::str_as_keyword(s) {
                Some(s) => FinalToken::lit_word(s),
                None => FinalToken::Word(SmolStr::new(s)),
            },
            PIdent(s) => FinalToken::PIdent(SmolStr::from(s)),
            AIdent(s) => FinalToken::AIdent(SmolStr::from(s)),
            LitBytes(s) => match Self::parse_bytes(s) {
                Ok(b) => Ok(FinalToken::LitBytes(b)),
                Err(e) => Err(String::from(e)),
            }?,
            LitDecInt(int) | LitHexInt(int) => FinalToken::LitInt(int),
            LitFPDec0Std(f) | LitFPDec1Std(f) | LitFPDec0Exp(f) | LitFPDec1Exp(f) => {
                FinalToken::LitFP(f)
            }
            LitFPHex0(fs) | LitFPHex1(fs) => FinalToken::LitFP(Self::parse_hex_fp(fs)?),

            LineComment | Space => panic!("Invalid token {self:?}"),
            Semi => FinalToken::Semi,
            Comma => FinalToken::Comma,
            Exclaim => FinalToken::Exclaim,
            DotDotEq => FinalToken::DotDotEq,
            Eq => FinalToken::Eq,
            LParen => FinalToken::LParen,
            RParen => FinalToken::RParen,
            LBracket => FinalToken::LBracket,
            RBracket => FinalToken::RBracket,
            LBrace => FinalToken::LBrace,
            RBrace => FinalToken::RBrace,
            LAngle => FinalToken::LAngle,
            RAngle => FinalToken::RAngle,
        };
        Ok(res)
    }
    fn str_as_keyword(s: &str) -> Option<&'static str> {
        let s = match s {
            "global" => "global",
            "constant" => "constant",
            "undef" => "undef",
            "poison" => "poison",
            "null" => "null",
            "true" => "true",
            "false" => "false",
            "zeroinitializer" => "zeroinitializer",
            "declare" => "declare",
            "define" => "define",
            "private" => "private",
            "internal" => "internal",
            "dso_local" => "dso_local",
            "i1" => "i1",
            "i8" => "i8",
            "i16" => "i16",
            "i32" => "i32",
            "i64" => "i64",
            "i128" => "i128",
            "float" => "float",
            "double" => "double",
            "void" => "void",
            _ => return None,
        };
        Some(s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FinalToken {
    /// integer literal (with sign; contains 10/16 radix)
    LitInt(i128),

    /// fp literal (with sign; contains 10/16 radix)
    LitFP(f64),

    /// string and bytes literal (converted into its real form)
    LitBytes(SmallVec<[u8; 16]>),

    /// identifiers (starts with '%')
    PIdent(SmolStr),

    /// identifiers (starts with '@')
    AIdent(SmolStr),

    /// words / keywords
    Word(SmolStr),

    Semi,
    Comma,
    Exclaim,
    DotDotEq,
    Eq,

    LParen,
    RParen,

    LBracket,
    RBracket,

    LBrace,
    RBrace,

    LAngle,
    RAngle,
}

impl FinalToken {
    pub fn lit_word(word: &'static str) -> Self {
        Self::Word(SmolStr::new_static(word))
    }
}

pub struct IRLexer<'lex> {
    spanned: logos::SpannedIter<'lex, PrimToken<'lex>>,
}

impl<'lex> Iterator for IRLexer<'lex> {
    type Item = (Result<FinalToken, String>, logos::Span);

    fn next(&mut self) -> Option<Self::Item> {
        let (res, span) = self.spanned.next()?;
        let res = res
            .map_err(|()| String::from("Internal lexer error"))
            .and_then(|token| token.to_final());
        Some((res, span))
    }
}

impl<'lex> IRLexer<'lex> {
    pub fn new(source: &'lex str) -> Self {
        Self {
            spanned: logos::Lexer::new(source).spanned(),
        }
    }

    pub fn get_pos(&self) -> usize {
        self.spanned.span().end
    }
    pub fn get_source(&self) -> &'lex str {
        self.spanned.source()
    }
}

impl FinalToken {
    pub fn lexer<'lex>(source: &'lex str) -> IRLexer<'lex> {
        IRLexer::new(source)
    }

    pub fn is_word(&self, s: &str) -> bool {
        let Self::Word(ident) = self else {
            return false;
        };
        ident.as_str() == s
    }
    pub fn as_word(&self) -> Option<&str> {
        match self {
            Self::Word(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexer() {
        let lexer = FinalToken::lexer(r#"hello c"Hello" 1.2345 0x5.6p0 true false @hello"#);
        for tok in lexer {
            println!("token {tok:?}");
        }
    }
}
