use chrono::{DateTime, Local};

/// Nombre de la carpeta destino para una fecha de modificación dada.
pub fn folder_for(datetime: DateTime<Local>) -> String {
    datetime.format("Y%Y-M%m").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn folder_for_formats_year_and_zero_padded_month() {
        let dt = Local.with_ymd_and_hms(2026, 7, 18, 12, 0, 0).unwrap();
        assert_eq!(folder_for(dt), "Y2026-M07");

        let dt = Local.with_ymd_and_hms(1999, 12, 31, 23, 59, 59).unwrap();
        assert_eq!(folder_for(dt), "Y1999-M12");
    }
}
