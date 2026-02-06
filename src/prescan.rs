//! Lightweight pre-scan to find disambiguating dates in large datasets.
//!
//! When the main inference path samples ~1000 dates via `step_by`, it can miss
//! the rare disambiguating date (value > 12 at a day/month position) that proves
//! DD/MM vs MM/DD ordering. This module scans ALL dates with minimal work —
//! just byte-level digit extraction — to locate such dates.

/// Scan all dates for disambiguating indices.
///
/// A "disambiguating" date has a 1-2 digit numeric segment with value > 12
/// at numeric position 0 or 1 (the two positions that could be day-or-month).
/// Four-digit segments (years) are skipped.
///
/// Returns `[Option<usize>; 2]` — one representative date index per numeric
/// position (0 and 1). Short-circuits once both positions are covered.
pub fn find_disambiguating_indices<S: AsRef<str>>(dates: &[S]) -> [Option<usize>; 2] {
    let mut result: [Option<usize>; 2] = [None; 2];

    for (idx, date) in dates.iter().enumerate() {
        let bytes = date.as_ref().as_bytes();
        let mut num_pos: usize = 0; // which numeric segment we're on
        let mut i = 0;

        while i < bytes.len() && num_pos < 2 {
            if bytes[i].is_ascii_digit() {
                // Collect consecutive digits
                let start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                let digit_len = i - start;

                // Skip 4-digit years
                if digit_len == 4 {
                    continue;
                }

                if digit_len == 1 || digit_len == 2 {
                    // Parse the 1-2 digit value
                    let val = if digit_len == 1 {
                        (bytes[start] - b'0') as u32
                    } else {
                        (bytes[start] - b'0') as u32 * 10 + (bytes[start + 1] - b'0') as u32
                    };

                    if val > 12 && result[num_pos].is_none() {
                        result[num_pos] = Some(idx);
                    }

                    num_pos += 1;
                } else {
                    // 3-digit or 5+ digit segment — skip, not a date component
                    num_pos += 1;
                }
            } else {
                i += 1;
            }
        }

        // Short-circuit once both positions are covered
        if result[0].is_some() && result[1].is_some() {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disambiguating_position_0() {
        // "15/03/2025" has 15 at position 0
        let dates = vec!["01/02/2025", "01/02/2025", "15/03/2025"];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[0], Some(2)); // index 2 has 15 at pos 0
    }

    #[test]
    fn test_disambiguating_position_1() {
        // "01/15/2025" has 15 at position 1
        let dates = vec!["01/02/2025", "01/02/2025", "03/15/2025"];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[1], Some(2)); // index 2 has 15 at pos 1
    }

    #[test]
    fn test_no_disambiguating() {
        // All values <= 12, no disambiguation possible
        let dates = vec!["01/02/2025", "03/04/2025", "05/06/2025"];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[0], None);
        assert_eq!(result[1], None);
    }

    #[test]
    fn test_skips_4_digit_years() {
        // "2025-01-15": first numeric segment is 2025 (4 digits, skipped)
        // Then 01 at pos 0, 15 at pos 1
        let dates = vec!["2025-01-15"];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[0], None); // 01 <= 12
        assert_eq!(result[1], Some(0)); // 15 > 12
    }

    #[test]
    fn test_short_circuits() {
        // First date covers pos 0, second covers pos 1
        let dates = vec!["15/02/2025", "01/20/2025", "99/99/9999"];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[0], Some(0));
        assert_eq!(result[1], Some(1));
        // Third date is never reached (short-circuit)
    }

    #[test]
    fn test_both_positions_same_date() {
        // "25/31/2025": 25 at pos 0, 31 at pos 1
        let dates = vec!["01/02/2025", "25/31/2025"];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[0], Some(1));
        assert_eq!(result[1], Some(1));
    }

    #[test]
    fn test_single_digit_values() {
        // "5/1/2025": single-digit segments, both <= 12
        // "5/15/2025": 5 at pos 0 (<=12), 15 at pos 1 (>12)
        let dates = vec!["5/1/2025", "5/15/2025"];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[0], None); // 5 <= 12
        assert_eq!(result[1], Some(1)); // 15 > 12
    }

    #[test]
    fn test_empty_input() {
        let dates: Vec<&str> = vec![];
        let result = find_disambiguating_indices(&dates);
        assert_eq!(result[0], None);
        assert_eq!(result[1], None);
    }
}
