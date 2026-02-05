//! Generate strptime format strings from resolved tokens

use crate::constraints::TokenType;
use crate::tokenizer::Token;

/// Convert resolved tokens to a strptime format string
pub fn to_strptime(tokens: &[Token], resolved_types: &[TokenType]) -> String {
    let mut format = String::new();

    for (token, token_type) in tokens.iter().zip(resolved_types.iter()) {
        match token_type {
            TokenType::Separator(c) => {
                // Escape special characters in strptime
                match c {
                    '%' => format.push_str("%%"),
                    _ => format.push(*c),
                }
            }
            TokenType::Unknown => {
                // Keep original value as literal
                format.push_str(&token.value);
            }
            _ => {
                format.push_str(token_type.strptime_format());
            }
        }
    }

    format
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokenizer::tokenize;

    #[test]
    fn test_strptime_dmy() {
        let tokens = tokenize("15/03/2025").unwrap();
        let resolved = vec![
            TokenType::Day,
            TokenType::Separator('/'),
            TokenType::Month,
            TokenType::Separator('/'),
            TokenType::Year4,
        ];
        assert_eq!(to_strptime(&tokens, &resolved), "%d/%m/%Y");
    }

    #[test]
    fn test_strptime_iso() {
        let tokens = tokenize("2025-01-15").unwrap();
        let resolved = vec![
            TokenType::Year4,
            TokenType::Separator('-'),
            TokenType::Month,
            TokenType::Separator('-'),
            TokenType::Day,
        ];
        assert_eq!(to_strptime(&tokens, &resolved), "%Y-%m-%d");
    }

    #[test]
    fn test_strptime_with_month_name() {
        let tokens = tokenize("15 Jan 2025").unwrap();
        let resolved = vec![
            TokenType::Day,
            TokenType::Separator(' '),
            TokenType::MonthNameShort,
            TokenType::Separator(' '),
            TokenType::Year4,
        ];
        assert_eq!(to_strptime(&tokens, &resolved), "%d %b %Y");
    }

    #[test]
    fn test_strptime_with_time() {
        let tokens = tokenize("2025-01-15 10:30:00").unwrap();
        let resolved = vec![
            TokenType::Year4,
            TokenType::Separator('-'),
            TokenType::Month,
            TokenType::Separator('-'),
            TokenType::Day,
            TokenType::Separator(' '),
            TokenType::Hour24,
            TokenType::Separator(':'),
            TokenType::Minute,
            TokenType::Separator(':'),
            TokenType::Second,
        ];
        assert_eq!(to_strptime(&tokens, &resolved), "%Y-%m-%d %H:%M:%S");
    }
}
