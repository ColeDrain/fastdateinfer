//! Tokenizer for date strings

use crate::constraints::{
    is_separator, possible_types_for_number, token_type_for_text, TokenType,
};
use crate::error::{DateInferError, Result};
use smallvec::SmallVec;

/// Compact storage for possible token types (inline up to 6 types, no heap allocation)
pub type TypeSet = SmallVec<[TokenType; 6]>;

/// A token extracted from a date string
#[derive(Debug, Clone)]
pub struct Token {
    /// The original string value
    pub value: String,
    /// Position in the original string
    pub position: u16,
    /// Possible token types based on constraints (inline storage, no heap for ≤6 types)
    pub possible_types: TypeSet,
    /// The parsed numeric value (if applicable)
    pub numeric_value: Option<u32>,
}

impl Token {
    /// Create a new separator token
    fn separator(c: char, position: usize) -> Self {
        let mut types = TypeSet::new();
        types.push(TokenType::Separator(c));
        Self {
            value: c.to_string(),
            position: position as u16,
            possible_types: types,
            numeric_value: None,
        }
    }

    /// Create a new numeric token
    fn numeric(value: &str, position: usize) -> Self {
        let parsed: Option<u32> = value.parse().ok();
        let types = if let Some(num) = parsed {
            possible_types_for_number(num, value.len())
        } else {
            let mut set = TypeSet::new();
            set.push(TokenType::Unknown);
            set
        };
        Self {
            value: value.to_string(),
            position: position as u16,
            possible_types: types,
            numeric_value: parsed,
        }
    }

    /// Create a new text token
    fn text(value: &str, position: usize) -> Self {
        let token_type = token_type_for_text(value);
        let mut types = TypeSet::new();
        types.push(token_type);
        Self {
            value: value.to_string(),
            position: position as u16,
            possible_types: types,
            numeric_value: None,
        }
    }

    /// Check if this token is a separator
    pub fn is_separator(&self) -> bool {
        self.possible_types.iter().any(|t| matches!(t, TokenType::Separator(_)))
    }

    /// Check if this token could be a day
    pub fn could_be_day(&self) -> bool {
        self.possible_types.iter().any(|t| matches!(t, TokenType::Day | TokenType::DayOrMonth))
    }

    /// Check if this token could be a month
    pub fn could_be_month(&self) -> bool {
        self.possible_types.iter().any(|t| matches!(t,
            TokenType::Month | TokenType::DayOrMonth | TokenType::MonthName | TokenType::MonthNameShort
        ))
    }

    /// Check if this token can ONLY be a day (value > 12)
    pub fn must_be_day(&self) -> bool {
        self.possible_types.contains(&TokenType::Day)
            && !self.possible_types.iter().any(|t| matches!(t, TokenType::DayOrMonth | TokenType::Month))
    }
}

/// Tokenize a date string into components
pub fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    let mut position = 0;

    while let Some(&c) = chars.peek() {
        if is_separator(c) {
            tokens.push(Token::separator(c, position));
            chars.next();
            position += 1;
        } else if c.is_ascii_digit() {
            // Collect all consecutive digits
            let start = position;
            let mut num_str = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_ascii_digit() {
                    num_str.push(c);
                    chars.next();
                    position += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token::numeric(&num_str, start));
        } else if c.is_alphabetic() {
            // Collect all consecutive letters
            let start = position;
            let mut text = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphabetic() {
                    text.push(c);
                    chars.next();
                    position += 1;
                } else {
                    break;
                }
            }
            tokens.push(Token::text(&text, start));
        } else if c == '+' || c == '-' {
            // Could be timezone offset like +05:30
            let start = position;
            let sign = c;
            chars.next();
            position += 1;

            // Check if followed by digits (timezone offset)
            if chars.peek().is_some_and(|c| c.is_ascii_digit()) {
                let mut offset = String::from(sign);
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == ':' {
                        offset.push(c);
                        chars.next();
                        position += 1;
                    } else {
                        break;
                    }
                }
                let mut types = TypeSet::new();
                types.push(TokenType::TzOffset);
                tokens.push(Token {
                    value: offset,
                    position: start as u16,
                    possible_types: types,
                    numeric_value: None,
                });
            } else {
                // Just a standalone sign, treat as separator
                tokens.push(Token::separator(sign, start));
            }
        } else {
            // Skip unknown characters
            chars.next();
            position += 1;
        }
    }

    if tokens.is_empty() {
        return Err(DateInferError::TokenizeError(input.to_string()));
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_dmy_slash() {
        let tokens = tokenize("15/03/2025").unwrap();
        assert_eq!(tokens.len(), 5);
        assert_eq!(tokens[0].value, "15");
        assert!(tokens[0].must_be_day()); // 15 > 12
        assert_eq!(tokens[1].value, "/");
        assert_eq!(tokens[2].value, "03");
        assert!(tokens[2].could_be_month());
        assert_eq!(tokens[4].value, "2025");
    }

    #[test]
    fn test_tokenize_iso() {
        let tokens = tokenize("2025-01-15").unwrap();
        assert_eq!(tokens.len(), 5);
        assert!(tokens[0].possible_types.contains(&TokenType::Year4));
    }

    #[test]
    fn test_tokenize_with_month_name() {
        let tokens = tokenize("15 Jan 2025").unwrap();
        assert_eq!(tokens.len(), 5);
        assert!(tokens[2].possible_types.contains(&TokenType::MonthNameShort));
    }

    #[test]
    fn test_tokenize_with_time() {
        let tokens = tokenize("2025-01-15T10:30:00").unwrap();
        assert!(tokens.len() > 5);
        // T should be a separator
        assert!(tokens.iter().any(|t| t.value == "T"));
    }

    #[test]
    fn test_tokenize_timezone() {
        let tokens = tokenize("2025-01-15T10:30:00+05:30").unwrap();
        assert!(tokens.iter().any(|t| t.possible_types.contains(&TokenType::TzOffset)));
    }
}
