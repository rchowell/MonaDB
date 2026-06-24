//! ANSI syntax highlighting for MonaDB SQL input lines.

use std::fmt::Write as _;

use crate::lexer::{SqlLexer, Token};

const RESET: &str = "\x1b[0m";
const KEYWORD: &str = "\x1b[1;33m";
const LITERAL: &str = "\x1b[36m";
const SYMBOL: &str = "\x1b[2m";

/// Returns `line` with ANSI escape sequences marking keywords, literals, and symbols.
pub fn highlight_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 32);
    let mut prev = 0;
    for item in SqlLexer::new(line) {
        let Ok((start, tok, end)) = item else {
            continue;
        };
        let _ = write!(out, "{}", &line[prev..start]);
        let style = style_for(&tok);
        let _ = write!(out, "{style}{}{RESET}", &line[start..end]);
        prev = end;
    }
    let _ = write!(out, "{}", &line[prev..]);
    out
}

fn style_for(tok: &Token) -> &'static str {
    match tok {
        Token::Null
        | Token::True
        | Token::False
        | Token::Number(_)
        | Token::String(_) => LITERAL,
        Token::TypeBool
        | Token::TypeInt
        | Token::TypeFloat
        | Token::TypeNumber
        | Token::TypeString
        | Token::TypeObject
        | Token::TypeArray
        | Token::TypeAny
        | Token::All
        | Token::And
        | Token::As
        | Token::Asc
        | Token::At
        | Token::Begin
        | Token::Between
        | Token::By
        | Token::Clear
        | Token::Commit
        | Token::Copy
        | Token::Create
        | Token::Delete
        | Token::Desc
        | Token::Drop
        | Token::Exists
        | Token::Group
        | Token::Having
        | Token::In
        | Token::Is
        | Token::Limit
        | Token::From
        | Token::Insert
        | Token::Into
        | Token::Not
        | Token::Or
        | Token::Order
        | Token::Pivot
        | Token::Rollback
        | Token::Select
        | Token::Table
        | Token::To
        | Token::Unknown
        | Token::Unpivot
        | Token::Where => KEYWORD,
        Token::Ast
        | Token::Comma
        | Token::Colon
        | Token::Dollar
        | Token::Question
        | Token::DotDot
        | Token::Ellipsis
        | Token::Eq
        | Token::Gt
        | Token::Ge
        | Token::Lt
        | Token::Le
        | Token::Minus
        | Token::Ne
        | Token::Percent
        | Token::Plus
        | Token::Period
        | Token::Pipe
        | Token::SemiColon
        | Token::Sol
        | Token::ParenL
        | Token::ParenR
        | Token::BraceL
        | Token::BraceR
        | Token::BracketL
        | Token::BracketR => SYMBOL,
        Token::NumberedParam(_) | Token::NamedParam(_) | Token::Identifier(_) => RESET,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_keywords() {
        let out = highlight_line("select * from t;");
        assert!(out.contains(KEYWORD));
        assert!(out.contains("select"));
    }

    #[test]
    fn highlights_string_literals() {
        let out = highlight_line("where name = 'alice';");
        assert!(out.contains(LITERAL));
        assert!(out.contains("'alice'"));
    }
}
