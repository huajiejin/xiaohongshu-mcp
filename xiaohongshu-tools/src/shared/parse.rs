use chrono::{Datelike, TimeZone};

pub fn parse_count(s: &str) -> Option<u64> {
    let s = s.trim().replace(',', "");
    if s.is_empty() {
        return None;
    }

    let (num_part, multiplier) = parse_scaled_value(&s)?;
    let base: f64 = num_part.parse().ok()?;
    let result = base * multiplier;
    if result >= 0.0 {
        Some(result as u64)
    } else {
        None
    }
}

fn parse_scaled_value(s: &str) -> Option<(String, f64)> {
    let suffixes: &[(&str, f64)] = &[
        ("亿", 1_0000_0000.0),
        ("万", 1_0000.0),
        ("w", 1_0000.0),
        ("k", 1_000.0),
        ("m", 1_000_000.0),
        ("b", 1_000_000_000.0),
    ];

    for (suffix, mult) in suffixes {
        if let Some(stripped) = s.to_lowercase().strip_suffix(suffix)
            && !stripped.is_empty()
        {
            return Some((stripped.to_string(), *mult));
        }
    }

    Some((s.to_string(), 1.0))
}

pub fn parse_publish_time(
    raw: &str,
    reference: chrono::DateTime<chrono::Local>,
) -> Option<chrono::DateTime<chrono::Local>> {
    let s = raw.trim();

    if let Some(stripped) = s.strip_suffix("分钟前") {
        let mins: i64 = stripped.parse().ok()?;
        return Some(reference - chrono::Duration::minutes(mins));
    }

    if let Some(stripped) = s.strip_suffix("小时前") {
        let hours: i64 = stripped.parse().ok()?;
        return Some(reference - chrono::Duration::hours(hours));
    }

    if let Some(stripped) = s.strip_suffix("天前") {
        let days: i64 = stripped.parse().ok()?;
        return Some(reference - chrono::Duration::days(days));
    }

    if let Some(stripped) = s.strip_suffix("周前") {
        let weeks: i64 = stripped.parse().ok()?;
        return Some(reference - chrono::Duration::weeks(weeks));
    }

    if let Some(stripped) = s.strip_suffix("个月前") {
        let months: u32 = stripped.parse().ok()?;
        return reference.checked_sub_months(chrono::Months::new(months));
    }

    if s == "昨天" {
        return Some(reference - chrono::Duration::days(1));
    }

    if s == "前天" {
        return Some(reference - chrono::Duration::days(2));
    }

    if s == "今天" {
        return Some(reference);
    }

    if let Ok(date) = chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let dt = date.and_hms_opt(0, 0, 0)?;
        return chrono::Local.from_local_datetime(&dt).single();
    }

    if let Some((month_str, day_str)) = s.split_once('-')
        && let (Ok(month), Ok(day)) = (month_str.parse::<u32>(), day_str.parse::<u32>())
        && (1..=12).contains(&month)
        && (1..=31).contains(&day)
    {
        let year = reference.year();
        if let Some(dt) =
            chrono::NaiveDate::from_ymd_opt(year, month, day).and_then(|d| d.and_hms_opt(0, 0, 0))
        {
            return chrono::Local.from_local_datetime(&dt).single();
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ymd_hms(
        year: i32,
        month: u32,
        day: u32,
        h: u32,
        m: u32,
        s: u32,
    ) -> chrono::DateTime<chrono::Local> {
        chrono::Local
            .with_ymd_and_hms(year, month, day, h, m, s)
            .single()
            .unwrap()
    }

    fn assert_pub_time(input: &str, ref_time: chrono::DateTime<chrono::Local>, expected_str: &str) {
        let result = parse_publish_time(input, ref_time);
        let expected = chrono::Local
            .with_ymd_and_hms(
                expected_str[..4].parse().unwrap(),
                expected_str[5..7].parse().unwrap(),
                expected_str[8..10].parse().unwrap(),
                expected_str[11..13].parse().unwrap(),
                expected_str[14..16].parse().unwrap(),
                expected_str[17..19].parse().unwrap(),
            )
            .single()
            .unwrap();
        assert_eq!(result, Some(expected));
    }

    #[test]
    fn test_parse_count_plain_integer() {
        assert_eq!(parse_count("42"), Some(42));
        assert_eq!(parse_count("0"), Some(0));
    }

    #[test]
    fn test_parse_count_with_commas() {
        assert_eq!(parse_count("1,430"), Some(1430));
        assert_eq!(parse_count("1,000,000"), Some(1_000_000));
    }

    #[test]
    fn test_parse_count_chinese_suffix() {
        assert_eq!(parse_count("1.2万"), Some(12000));
        assert_eq!(parse_count("3万"), Some(30000));
        assert_eq!(parse_count("2亿"), Some(2_0000_0000));
    }

    #[test]
    fn test_parse_count_english_suffix() {
        assert_eq!(parse_count("1.4k"), Some(1400));
        assert_eq!(parse_count("2.5M"), Some(2_500_000));
        assert_eq!(parse_count("1w"), Some(10000));
    }

    #[test]
    fn test_parse_count_invalid() {
        assert_eq!(parse_count(""), None);
        assert_eq!(parse_count("abc"), None);
    }

    #[test]
    fn test_parse_publish_time_mm_dd() {
        assert_pub_time(
            "03-27",
            ymd_hms(2026, 4, 3, 12, 0, 0),
            "2026-03-27T00:00:00",
        );
    }

    #[test]
    fn test_parse_publish_time_days_ago() {
        assert_pub_time(
            "2天前",
            ymd_hms(2026, 4, 3, 12, 30, 0),
            "2026-04-01T12:30:00",
        );
    }

    #[test]
    fn test_parse_publish_time_hours_ago() {
        assert_pub_time(
            "11小时前",
            ymd_hms(2026, 4, 3, 14, 0, 0),
            "2026-04-03T03:00:00",
        );
    }

    #[test]
    fn test_parse_publish_time_minutes_ago() {
        assert_pub_time(
            "30分钟前",
            ymd_hms(2026, 4, 3, 12, 30, 0),
            "2026-04-03T12:00:00",
        );
    }

    #[test]
    fn test_parse_publish_time_yesterday() {
        assert_pub_time("昨天", ymd_hms(2026, 4, 3, 15, 0, 0), "2026-04-02T15:00:00");
    }

    #[test]
    fn test_parse_publish_time_weeks_ago() {
        assert_pub_time(
            "1周前",
            ymd_hms(2026, 4, 3, 10, 0, 0),
            "2026-03-27T10:00:00",
        );
    }

    #[test]
    fn test_parse_publish_time_months_ago() {
        assert_pub_time(
            "3个月前",
            ymd_hms(2026, 4, 3, 10, 0, 0),
            "2026-01-03T10:00:00",
        );
    }

    #[test]
    fn test_parse_publish_time_invalid() {
        assert_eq!(
            parse_publish_time("some random text", chrono::Local::now()),
            None
        );
    }
}
