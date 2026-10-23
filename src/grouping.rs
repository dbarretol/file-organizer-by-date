use chrono::{DateTime, Local};

/// Level of detail of the destination folders.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Granularity {
    /// `Y2024`
    Year,
    /// `Y2024-M01`
    #[default]
    Month,
    /// `Y2024-M01-D15`
    Day,
}

/// Destination folder name for a given date.
pub fn folder_for(datetime: DateTime<Local>, granularity: Granularity) -> String {
    let pattern = match granularity {
        Granularity::Year => "Y%Y",
        Granularity::Month => "Y%Y-M%m",
        Granularity::Day => "Y%Y-M%m-D%d",
    };
    datetime.format(pattern).to_string()
}

/// Does `name` look like a destination folder of any granularity (`Y2024`,
/// `Y2024-M01`, `Y2024-M01-D15`)? Recursive mode doesn't descend into them:
/// they're already organized, so running the tool twice changes nothing.
pub(crate) fn is_date_folder(name: &str) -> bool {
    fn take<'a>(s: &'a str, prefix: &str, digits: usize) -> Option<&'a str> {
        let rest = s.strip_prefix(prefix)?;
        let head = rest.get(..digits)?;
        head.bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| &rest[digits..])
    }

    let Some(mut rest) = take(name, "Y", 4) else {
        return false;
    };
    for (prefix, digits) in [("-M", 2), ("-D", 2)] {
        match take(rest, prefix, digits) {
            Some(r) => rest = r,
            None => break,
        }
    }
    rest.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn folder_for_formats_year_and_zero_padded_month() {
        let dt = Local.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();
        assert_eq!(folder_for(dt, Granularity::Month), "Y2026-M07");

        let dt = Local.with_ymd_and_hms(1999, 12, 31, 23, 59, 59).unwrap();
        assert_eq!(folder_for(dt, Granularity::Month), "Y1999-M12");
    }

    #[test]
    fn folder_for_supports_every_granularity() {
        let dt = Local.with_ymd_and_hms(2024, 1, 5, 8, 0, 0).unwrap();
        assert_eq!(folder_for(dt, Granularity::Year), "Y2024");
        assert_eq!(folder_for(dt, Granularity::Month), "Y2024-M01");
        assert_eq!(folder_for(dt, Granularity::Day), "Y2024-M01-D05");
    }

    #[test]
    fn is_date_folder_recognizes_every_granularity_only() {
        for name in ["Y2024", "Y2024-M01", "Y2024-M01-D15"] {
            assert!(is_date_folder(name), "{name}");
        }
        for name in [
            "Y202",
            "Y20245",
            "Y2024-M1",
            "Y2024-D15",
            "Y2024-M01-",
            "y2024",
            "Photos",
            "",
        ] {
            assert!(!is_date_folder(name), "{name}");
        }
    }

    #[test]
    fn month_is_the_default_granularity() {
        assert_eq!(Granularity::default(), Granularity::Month);
    }
}
