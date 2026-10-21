//! The lexer (metamodel §16.1): a lossless token stream. Concatenating every token's text
//! gives back the input byte for byte, including whitespace, comments and bad bytes.

use winnow::combinator::{alt, not, preceded, repeat, terminated};
use winnow::error::{ContextError, ErrMode};
use winnow::prelude::*;
use winnow::token::{any, one_of, take_till, take_while};

/// The kinds of token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenKind {
    /// Spaces and tabs.
    Whitespace,
    /// `\n` or `\r\n`.
    Newline,
    /// `#` to the end of the line.
    Comment,
    /// `#n`, the occurrence index of a note's arrow target (§16.2 `note_target`).
    Occurrence,
    /// `"…"`, with escapes.
    Str,
    /// `"""…"""`.
    Triple,
    /// A run of `[A-Za-z0-9_.@/-]` that does not contain `->`: keywords, keys, node numbers,
    /// endpoints such as `1O1` or `-1O2`.
    Word,
    /// `->`.
    Arrow,
    /// `{`.
    LBrace,
    /// `}`.
    RBrace,
    /// `:`.
    Colon,
    /// `,`.
    Comma,
    /// `|`.
    Pipe,
    /// `=`.
    Eq,
    /// `[`.
    LBracket,
    /// `]`.
    RBracket,
    /// A string with no closing quote.
    UnterminatedStr,
    /// One byte or character the grammar does not use.
    Unknown,
}

impl TokenKind {
    /// Whitespace, newlines and comments: kept in the tree, skipped by the parser.
    pub fn is_trivia(self) -> bool {
        matches!(self, Self::Whitespace | Self::Newline | Self::Comment)
    }
}

/// A token: its kind and its byte range in the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    /// The kind.
    pub kind: TokenKind,
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset one past the last byte.
    pub end: u32,
}

impl Token {
    /// The token's text in `src`.
    pub fn text<'s>(&self, src: &'s str) -> &'s str {
        &src[self.start as usize..self.end as usize]
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '@' | '/' | '-')
}

fn word<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
    let part = alt((
        take_while(1.., |c: char| is_word_char(c) && c != '-'),
        terminated("-", not(">")),
    ));
    repeat::<_, _, (), _, _>(1.., part).take().parse_next(input)
}

/// The inside of a `"…"` string: plain runs and `\x` escapes, never a raw newline.
fn string_body(input: &mut &str) -> ModalResult<()> {
    let plain = take_till(1.., ['"', '\\', '\n']).void();
    let escaped = preceded('\\', any).void();
    repeat(0.., alt((plain, escaped))).parse_next(input)
}

fn string<'s>(input: &mut &'s str) -> ModalResult<(TokenKind, &'s str)> {
    let closed = ('"', string_body, '"').take().map(|t| (TokenKind::Str, t));
    let open = ('"', string_body)
        .take()
        .map(|t| (TokenKind::UnterminatedStr, t));
    alt((closed, open)).parse_next(input)
}

fn triple<'s>(input: &mut &'s str) -> ModalResult<(TokenKind, &'s str)> {
    let closed = (
        "\"\"\"",
        repeat::<_, _, (), _, _>(0.., (not("\"\"\""), any)),
        "\"\"\"",
    )
        .take()
        .map(|t| (TokenKind::Triple, t));
    let open = ("\"\"\"", repeat::<_, _, (), _, _>(0.., any))
        .take()
        .map(|t| (TokenKind::UnterminatedStr, t));
    alt((closed, open)).parse_next(input)
}

/// `#` and digits, then a delimiter: the occurrence suffix of a note target. Anything else
/// after `#` is a comment.
fn occurrence<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
    terminated(
        ('#', take_while(1.., |c: char| c.is_ascii_digit())).take(),
        not(one_of(|c: char| {
            !matches!(c, ' ' | '\t' | '\r' | '\n' | ',' | '}')
        })),
    )
    .parse_next(input)
}

fn kind(k: TokenKind) -> impl FnMut(&str) -> (TokenKind, &str) {
    move |t| (k, t)
}

fn comment<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
    ('#', take_till(0.., ['\r', '\n'])).take().parse_next(input)
}

fn one_char<'s>(input: &mut &'s str) -> ModalResult<&'s str> {
    any::<_, ContextError>
        .take()
        .parse_next(input)
        .map_err(ErrMode::Backtrack)
}

/// One token, chosen by its first character.
fn token<'s>(input: &mut &'s str) -> ModalResult<(TokenKind, &'s str)> {
    let punct = |k| one_char.map(kind(k));
    match input.chars().next() {
        Some(' ' | '\t') => take_while(1.., [' ', '\t'])
            .map(kind(TokenKind::Whitespace))
            .parse_next(input),
        Some('\n') => "\n".map(kind(TokenKind::Newline)).parse_next(input),
        Some('\r') if input.starts_with("\r\n") => {
            "\r\n".map(kind(TokenKind::Newline)).parse_next(input)
        }
        Some('#') => alt((
            occurrence.map(kind(TokenKind::Occurrence)),
            comment.map(kind(TokenKind::Comment)),
        ))
        .parse_next(input),
        Some('"') => alt((triple, string)).parse_next(input),
        Some('-') if input.starts_with("->") => "->".map(kind(TokenKind::Arrow)).parse_next(input),
        Some(c) if is_word_char(c) => word.map(kind(TokenKind::Word)).parse_next(input),
        Some('{') => punct(TokenKind::LBrace).parse_next(input),
        Some('}') => punct(TokenKind::RBrace).parse_next(input),
        Some(':') => punct(TokenKind::Colon).parse_next(input),
        Some(',') => punct(TokenKind::Comma).parse_next(input),
        Some('|') => punct(TokenKind::Pipe).parse_next(input),
        Some('=') => punct(TokenKind::Eq).parse_next(input),
        Some('[') => punct(TokenKind::LBracket).parse_next(input),
        Some(']') => punct(TokenKind::RBracket).parse_next(input),
        _ => punct(TokenKind::Unknown).parse_next(input),
    }
}

/// Split `src` into tokens. Never fails: unexpected input becomes `Unknown` or
/// `UnterminatedStr` tokens, which the parser reports.
pub fn lex(src: &str) -> Vec<Token> {
    let mut rest = src;
    let mut out = Vec::new();
    while !rest.is_empty() {
        let before = rest.len();
        let Ok((kind, _)) = token.parse_next(&mut rest) else {
            break;
        };
        let start = src.len() - before;
        let end = src.len() - rest.len();
        out.push(Token {
            kind,
            start: u32::try_from(start).unwrap_or(u32::MAX),
            end: u32::try_from(end).unwrap_or(u32::MAX),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(src: &str) -> Vec<(TokenKind, &str)> {
        lex(src)
            .into_iter()
            .filter(|t| !t.kind.is_trivia())
            .map(|t| (t.kind, t.text(src)))
            .collect()
    }

    #[test]
    fn statement_tokens() {
        use TokenKind::*;
        assert_eq!(
            kinds("arrow @draft-model : 2O1 -> fork {"),
            [
                (Word, "arrow"),
                (Word, "@draft-model"),
                (Colon, ":"),
                (Word, "2O1"),
                (Arrow, "->"),
                (Word, "fork"),
                (LBrace, "{")
            ]
        );
        assert_eq!(
            kinds(r#"box 0 "Top" key=top c="AK1""#),
            [
                (Word, "box"),
                (Word, "0"),
                (Str, "\"Top\""),
                (Word, "key"),
                (Eq, "="),
                (Word, "top"),
                (Word, "c"),
                (Eq, "="),
                (Str, "\"AK1\"")
            ]
        );
        assert_eq!(
            kinds("-1O2->[I1]"),
            [
                (Word, "-1O2"),
                (Arrow, "->"),
                (LBracket, "["),
                (Word, "I1"),
                (RBracket, "]")
            ]
        );
        assert_eq!(
            kinds("SEC/@verify A21.3 A-0"),
            [(Word, "SEC/@verify"), (Word, "A21.3"), (Word, "A-0")]
        );
    }

    // §16.1: comments run from `#` to the end of the line; `#n` before a delimiter is an
    // occurrence index.
    #[test]
    fn comments_and_occurrences() {
        use TokenKind::*;
        let src = "1O1 -> 2I1 #1, box 2  # a comment #3\n#0x\n";
        let all: Vec<(TokenKind, &str)> = lex(src)
            .into_iter()
            .map(|t| (t.kind, t.text(src)))
            .collect();
        assert!(all.contains(&(Occurrence, "#1")));
        assert!(all.contains(&(Comment, "# a comment #3")));
        assert!(all.contains(&(Comment, "#0x")));
    }

    #[test]
    fn strings() {
        use TokenKind::*;
        assert_eq!(kinds(r#""a \"b\" \\ c""#), [(Str, r#""a \"b\" \\ c""#)]);
        assert_eq!(
            kinds("\"\"\"line 1\n  line \"2\"\n\"\"\""),
            [(Triple, "\"\"\"line 1\n  line \"2\"\n\"\"\"")]
        );
        assert_eq!(
            kinds("\"open\nnext"),
            [(UnterminatedStr, "\"open"), (Word, "next")]
        );
        assert_eq!(
            kinds("\"\"\"never closed"),
            [(UnterminatedStr, "\"\"\"never closed")]
        );
        assert_eq!(kinds("é;"), [(Unknown, "é"), (Unknown, ";")]);
    }

    // FR-WS-06: the token stream is lossless.
    #[test]
    fn lossless() {
        for src in [
            "",
            "idef0 0.2\r\nmodel \"x\" {\n}\n",
            "\"\"\"\n",
            "#\n#",
            "a->b-c--d",
            "é\u{0}\t",
        ] {
            let joined: String = lex(src).iter().map(|t| t.text(src)).collect();
            assert_eq!(joined, src);
        }
    }
}
