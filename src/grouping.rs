use chrono::{DateTime, Local};

/// Nivel de detalle de las carpetas destino.
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

/// Nombre de la carpeta destino para una fecha de modificación dada.
pub fn folder_for(datetime: DateTime<Local>, granularity: Granularity) -> String {
    let pattern = match granularity {
        Granularity::Year => "Y%Y",
        Granularity::Month => "Y%Y-M%m",
        Granularity::Day => "Y%Y-M%m-D%d",
    };
    datetime.format(pattern).to_string()
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
    fn month_is_the_default_granularity() {
        assert_eq!(Granularity::default(), Granularity::Month);
    }
}
