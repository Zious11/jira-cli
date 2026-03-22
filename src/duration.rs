use anyhow::{bail, Result};

/// Parses a human-friendly duration string into seconds.
/// Supported formats: `30m`, `2h`, `1h30m`, `1d`, `1w`, `1w2d3h30m`
pub fn parse_duration(input: &str, hours_per_day: u64, days_per_week: u64) -> Result<u64> {
    let input = input.trim().to_lowercase();
    if input.is_empty() {
        bail!("Duration cannot be empty");
    }

    let mut total_seconds: u64 = 0;
    let mut current_num = String::new();
    let mut found_any = false;

    for ch in input.chars() {
        if ch.is_ascii_digit() {
            current_num.push(ch);
        } else {
            if current_num.is_empty() {
                bail!("Invalid duration format: \"{input}\". Expected format like 2h, 1h30m, 1d");
            }
            let num: u64 = current_num
                .parse()
                .map_err(|_| anyhow::anyhow!("Invalid number in duration: \"{input}\""))?;
            current_num.clear();
            found_any = true;

            match ch {
                'w' => total_seconds += num * days_per_week * hours_per_day * 3600,
                'd' => total_seconds += num * hours_per_day * 3600,
                'h' => total_seconds += num * 3600,
                'm' => total_seconds += num * 60,
                _ => bail!("Unknown duration unit '{ch}' in \"{input}\". Use w, d, h, or m"),
            }
        }
    }

    if !current_num.is_empty() {
        bail!(
            "Invalid duration format: \"{input}\". Number without unit — did you mean \"{input}m\" or \"{input}h\"?"
        );
    }

    if !found_any {
        bail!("Invalid duration format: \"{input}\"");
    }

    Ok(total_seconds)
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
    const HPD: u64 = 8;
    const DPW: u64 = 5;

    #[test]
    fn test_minutes() {
        assert_eq!(parse_duration("30m", HPD, DPW).unwrap(), 1800);
    }
    #[test]
    fn test_hours() {
        assert_eq!(parse_duration("2h", HPD, DPW).unwrap(), 7200);
    }
    #[test]
    fn test_hours_and_minutes() {
        assert_eq!(parse_duration("1h30m", HPD, DPW).unwrap(), 5400);
    }
    #[test]
    fn test_day() {
        assert_eq!(parse_duration("1d", HPD, DPW).unwrap(), 28800);
    }
    #[test]
    fn test_week() {
        assert_eq!(parse_duration("1w", HPD, DPW).unwrap(), 144000);
    }
    #[test]
    fn test_complex() {
        assert_eq!(
            parse_duration("1w2d3h30m", HPD, DPW).unwrap(),
            144000 + 57600 + 10800 + 1800
        );
    }
    #[test]
    fn test_empty_fails() {
        assert!(parse_duration("", HPD, DPW).is_err());
    }
    #[test]
    fn test_number_without_unit_fails() {
        let err = parse_duration("30", HPD, DPW).unwrap_err();
        assert!(err.to_string().contains("without unit"));
    }
    #[test]
    fn test_invalid_unit_fails() {
        assert!(parse_duration("5x", HPD, DPW).is_err());
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
        fn valid_single_units_always_parse(h in 1u64..100, unit in prop_oneof![Just("m"), Just("h"), Just("d"), Just("w")]) {
            let input = format!("{h}{unit}");
            let result = parse_duration(&input, 8, 5);
            prop_assert!(result.is_ok(), "Failed to parse: {}", input);
            prop_assert!(result.unwrap() > 0);
        }

        #[test]
        fn combined_units_always_parse(h in 0u64..24, m in 0u64..60) {
            if h == 0 && m == 0 { return Ok(()); }
            let input = if m == 0 { format!("{h}h") } else if h == 0 { format!("{m}m") } else { format!("{h}h{m}m") };
            let result = parse_duration(&input, 8, 5);
            prop_assert!(result.is_ok(), "Failed to parse: {}", input);
        }

        #[test]
        fn garbage_input_never_panics(s in "\\PC{1,20}") {
            let _ = parse_duration(&s, 8, 5);
        }

        #[test]
        fn format_roundtrip(seconds in (1u64..86400).prop_filter("divisible by 60", |s| s % 60 == 0)) {
            let formatted = format_duration(seconds);
            let reparsed = parse_duration(&formatted, 8, 5).unwrap();
            if seconds < 28800 {
                prop_assert_eq!(reparsed, seconds, "Roundtrip failed: {} -> {} -> {}", seconds, formatted, reparsed);
            }
        }
    }
}
