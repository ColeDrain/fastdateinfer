//! Token types and constraint logic for date components

use crate::tokenizer::TypeSet;

/// Types of tokens that can appear in a date string
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenType {
    // Date components
    Year4,          // 2025 (4 digits, 1900-2100 range)
    Year2,          // 25 (2 digits)
    Month,          // 01-12
    Day,            // 01-31
    MonthName,      // January, February, etc.
    MonthNameShort, // Jan, Feb, etc.
    WeekdayName,    // Monday, Tuesday, etc.
    WeekdayShort,   // Mon, Tue, etc.

    // Time components
    Hour24,    // 00-23
    Hour12,    // 01-12
    Minute,    // 00-59
    Second,    // 00-59
    Subsecond, // fractional seconds
    AmPm,      // AM, PM

    // Timezone
    TzOffset, // +05:30, -0800
    TzName,   // UTC, EST, IST
    TzZ,      // Z (UTC indicator)

    // Separators
    Separator(char), // /, -, ., :, T, space

    // Ambiguous (to be resolved)
    DayOrMonth, // Could be day or month (value 1-12)

    // Unknown
    Unknown,
}

impl TokenType {
    /// Returns the strptime format specifier for this token type
    pub fn strptime_format(&self) -> &'static str {
        match self {
            TokenType::Year4 => "%Y",
            TokenType::Year2 => "%y",
            TokenType::Month => "%m",
            TokenType::Day => "%d",
            TokenType::MonthName => "%B",
            TokenType::MonthNameShort => "%b",
            TokenType::WeekdayName => "%A",
            TokenType::WeekdayShort => "%a",
            TokenType::Hour24 => "%H",
            TokenType::Hour12 => "%I",
            TokenType::Minute => "%M",
            TokenType::Second => "%S",
            TokenType::Subsecond => "%f",
            TokenType::AmPm => "%p",
            TokenType::TzOffset => "%z",
            TokenType::TzName => "%Z",
            TokenType::TzZ => "Z",
            TokenType::Separator(_) => "", // Handled specially
            TokenType::DayOrMonth => "%d", // Default to day
            TokenType::Unknown => "",
        }
    }

    /// Check if this token type is a date component (not separator/unknown)
    pub fn is_date_component(&self) -> bool {
        !matches!(self, TokenType::Separator(_) | TokenType::Unknown)
    }
}

/// Short month names (case-insensitive matching)
pub const MONTH_NAMES_SHORT: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun",
    "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Full month names (case-insensitive matching)
pub const MONTH_NAMES_FULL: [&str; 12] = [
    "january", "february", "march", "april", "may", "june",
    "july", "august", "september", "october", "november", "december",
];

/// Short weekday names
pub const WEEKDAY_NAMES_SHORT: [&str; 7] = [
    "mon", "tue", "wed", "thu", "fri", "sat", "sun",
];

/// Full weekday names
pub const WEEKDAY_NAMES_FULL: [&str; 7] = [
    "monday", "tuesday", "wednesday", "thursday", "friday", "saturday", "sunday",
];

/// AM/PM indicators
pub const AMPM: [&str; 4] = ["am", "pm", "a.m.", "p.m."];

/// Determine possible token types for a numeric value
pub fn possible_types_for_number(value: u32, num_digits: usize) -> TypeSet {
    let mut types = TypeSet::new();

    match num_digits {
        1 | 2 => {
            // Could be day, month, hour, minute, second, or 2-digit year
            if (1..=12).contains(&value) {
                types.push(TokenType::DayOrMonth); // Ambiguous day/month
                types.push(TokenType::Hour12);
            }
            if (1..=31).contains(&value) {
                types.push(TokenType::Day);
            }
            if value <= 23 {
                types.push(TokenType::Hour24);
            }
            if value <= 59 {
                types.push(TokenType::Minute);
                types.push(TokenType::Second);
            }
            if num_digits == 2 && value <= 99 {
                types.push(TokenType::Year2);
            }
        }
        4 => {
            // Likely a year
            if (1900..=2100).contains(&value) {
                types.push(TokenType::Year4);
            }
            // Could also be HHMM time without separator
            if value <= 2359 && value % 100 <= 59 {
                // This is rare, skip for now
            }
        }
        _ => {
            types.push(TokenType::Unknown);
        }
    }

    if types.is_empty() {
        types.push(TokenType::Unknown);
    }

    types
}

/// Determine token type for a text value
pub fn token_type_for_text(text: &str) -> TokenType {
    let lower = text.to_lowercase();

    // Check month names
    if let Some(idx) = MONTH_NAMES_SHORT.iter().position(|&m| m == lower) {
        return if text.len() == 3 {
            TokenType::MonthNameShort
        } else {
            // Check if it's a full month name
            if MONTH_NAMES_FULL.get(idx).is_some_and(|&full| full == lower) {
                TokenType::MonthName
            } else {
                TokenType::MonthNameShort
            }
        };
    }

    if MONTH_NAMES_FULL.iter().any(|&m| m == lower) {
        return TokenType::MonthName;
    }

    // Check weekday names
    if WEEKDAY_NAMES_SHORT.iter().any(|&w| w == lower) {
        return TokenType::WeekdayShort;
    }

    if WEEKDAY_NAMES_FULL.iter().any(|&w| w == lower) {
        return TokenType::WeekdayName;
    }

    // Check AM/PM
    if AMPM.iter().any(|&a| a == lower) {
        return TokenType::AmPm;
    }

    // Check timezone indicator
    if lower == "z" {
        return TokenType::TzZ;
    }

    // Common timezone abbreviations
    if matches!(lower.as_str(), "utc" | "gmt" | "est" | "pst" | "cst" | "mst" | "ist" | "cet" | "wet" | "eet") {
        return TokenType::TzName;
    }

    TokenType::Unknown
}

/// Check if a character is a common date/time separator
pub fn is_separator(c: char) -> bool {
    matches!(c, '/' | '-' | '.' | ':' | ' ' | 'T' | ',' | '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_number_constraints() {
        // Value 15 can only be day (not month)
        let types = possible_types_for_number(15, 2);
        assert!(types.contains(&TokenType::Day));
        assert!(!types.contains(&TokenType::DayOrMonth));

        // Value 5 is ambiguous
        let types = possible_types_for_number(5, 2);
        assert!(types.contains(&TokenType::DayOrMonth));
    }

    #[test]
    fn test_month_name_detection() {
        assert_eq!(token_type_for_text("Jan"), TokenType::MonthNameShort);
        assert_eq!(token_type_for_text("January"), TokenType::MonthName);
        assert_eq!(token_type_for_text("JAN"), TokenType::MonthNameShort);
    }

    #[test]
    fn test_year_detection() {
        let types = possible_types_for_number(2025, 4);
        assert!(types.contains(&TokenType::Year4));
    }
}
