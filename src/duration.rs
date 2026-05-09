use anyhow::Result;

use crate::error::JrError;

/// Maximum byte length accepted by `parse_duration_validate`.
///
/// The longest realistic duration string a user would type is something like
/// "99w 99d 99h 99m" (17 bytes). 64 bytes gives generous headroom while
/// bounding allocation before any whitespace-strip or heap allocation occurs.
/// Inputs over this limit are rejected with exit code 64 (user error).
pub(crate) const MAX_DURATION_INPUT_LEN: usize = 64;

/// Validates a Jira worklog duration string syntactically.
///
/// Accepts compact (`1h30m`, `1w2d3h30m`) and space-separated forms
/// (`2d 3h 30m`, `1w 2d 3h 4m`). Unit characters are `w`, `d`, `h`, `m`.
/// Performs NO arithmetic — no `hours_per_day` or `days_per_week` parameters.
///
/// Returns `Ok(())` for valid syntax, `Err(JrError::UserError(...))` for invalid
/// input so callers get exit code 64.
pub fn parse_duration_validate(input: &str) -> Result<()> {
    if input.len() > MAX_DURATION_INPUT_LEN {
        return Err(JrError::UserError(format!(
            "Duration string too long ({} bytes; max {}). Use format: Nw Nd Nh Nm (e.g., 1w2d3h30m, 2d 3h, 30m).",
            input.len(),
            MAX_DURATION_INPUT_LEN
        ))
        .into());
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(JrError::UserError(
            "Duration cannot be empty. Use format: Nw Nd Nh Nm (e.g. 2h, 1d, 2d 3h 30m)"
                .to_string(),
        )
        .into());
    }

    // Normalise: remove whitespace between units so we can parse as a single token stream.
    let normalised: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    let input_lower = normalised.to_lowercase();

    let mut current_num = String::new();
    let mut found_any = false;

    for ch in input_lower.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else {
            if current_num.is_empty() {
                return Err(JrError::UserError(format!(
                    "Invalid duration \"{input}\": a unit letter appeared before any number. \
                     Use format: Nw Nd Nh Nm (e.g. 2h, 1d, 2d 3h 30m)"
                ))
                .into());
            }
            current_num.clear();
            found_any = true;

            match ch {
                'w' | 'd' | 'h' | 'm' => {}
                _ => {
                    return Err(JrError::UserError(format!(
                        "Unknown duration unit '{ch}' in \"{input}\". \
                         Use format: Nw Nd Nh Nm (e.g. 2h, 1d, 2d 3h 30m)"
                    ))
                    .into());
                }
            }
        }
    }

    if !current_num.is_empty() {
        return Err(JrError::UserError(format!(
            "Invalid duration \"{input}\": number without unit. \
             Use format: Nw Nd Nh Nm (e.g. 2h, 1d, 2d 3h 30m)"
        ))
        .into());
    }

    if !found_any {
        return Err(JrError::UserError(format!(
            "Invalid duration \"{input}\". Use format: Nw Nd Nh Nm (e.g. 2h, 1d, 2d 3h 30m)"
        ))
        .into());
    }

    Ok(())
}

/// Formats seconds into a human-readable duration string
pub fn format_duration(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;

    match (hours, minutes) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h{m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // WV2-SEC-01 regression pins: input length cap on parse_duration_validate

    #[test]
    fn test_parse_duration_validate_rejects_input_longer_than_max() {
        let too_long = "1d".repeat(40); // 80 bytes — over the 64-byte cap
        let result = parse_duration_validate(&too_long);
        assert!(result.is_err(), "input over cap must be rejected");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("too long"),
            "error must mention size; got: {}",
            err_msg
        );
        assert!(
            err_msg.contains("64"),
            "error must cite the cap; got: {}",
            err_msg
        );
    }

    #[test]
    fn test_parse_duration_validate_accepts_input_at_max_boundary() {
        // "1m" repeated 31 times = 62 bytes; under the 64-byte cap and syntactically valid
        let just_under_cap = "1m".repeat(31);
        assert_eq!(
            just_under_cap.len(),
            62,
            "precondition: input must be 62 bytes"
        );
        let result = parse_duration_validate(&just_under_cap);
        assert!(
            result.is_ok(),
            "input under cap with valid syntax must be accepted; got: {:?}",
            result
        );
    }

    #[test]
    fn test_format_minutes() {
        assert_eq!(format_duration(1800), "30m");
    }
    #[test]
    fn test_format_hours() {
        assert_eq!(format_duration(7200), "2h");
    }
    #[test]
    fn test_format_hours_and_minutes() {
        assert_eq!(format_duration(5400), "1h30m");
    }
}

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn format_roundtrip(seconds in (1u64..1440u64).prop_map(|m| m * 60)) {
            let formatted = format_duration(seconds);
            // Parse the formatted string back to seconds structurally (no inverse function).
            // format_duration only emits h and m tokens: "Xm", "Xh", or "XhYm".
            let mut reconstructed = 0u64;
            for (suffix, multiplier) in &[("h", 3600u64), ("m", 60u64)] {
                if let Some(pos) = formatted.find(suffix) {
                    let start = formatted[..pos]
                        .rfind(|c: char| !c.is_ascii_digit())
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    let n: u64 = formatted[start..pos].parse().unwrap_or(0);
                    reconstructed += n * multiplier;
                }
            }
            prop_assert_eq!(
                reconstructed,
                seconds,
                "format_duration({}) = {:?} did not round-trip to same seconds",
                seconds,
                formatted
            );
        }
    }
}
