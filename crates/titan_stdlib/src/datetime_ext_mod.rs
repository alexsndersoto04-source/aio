//! std::datetime — Aritmetica avanzada, timezones, formatos humanos.
//!
//! Extension del `datetime_mod` original. Backend: chrono + chrono-tz
//! para zonas horarias. Todas las fns aceptan segundos Unix (i64) como
//! representacion intermedia, para no exponer tipos de chrono al VM.

use chrono::{DateTime, Datelike, NaiveDate, NaiveDateTime, Offset, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

// ---------------- Componentes de una fecha ----------------

fn to_dt(ts: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(ts, 0).unwrap_or_else(|| Utc.timestamp_opt(0, 0).unwrap())
}

pub fn year(ts: i64) -> i64 {
    to_dt(ts).year() as i64
}
pub fn month(ts: i64) -> i64 {
    to_dt(ts).month() as i64
}
pub fn day(ts: i64) -> i64 {
    to_dt(ts).day() as i64
}
pub fn hour(ts: i64) -> i64 {
    to_dt(ts).hour() as i64
}
pub fn minute(ts: i64) -> i64 {
    to_dt(ts).minute() as i64
}
pub fn second(ts: i64) -> i64 {
    to_dt(ts).second() as i64
}
pub fn day_of_week(ts: i64) -> i64 {
    to_dt(ts).weekday().num_days_from_monday() as i64
}
pub fn day_of_year(ts: i64) -> i64 {
    to_dt(ts).ordinal() as i64
}
pub fn week_of_year(ts: i64) -> i64 {
    to_dt(ts).iso_week().week() as i64
}
pub fn quarter(ts: i64) -> i64 {
    let m = to_dt(ts).month();
    (((m - 1) / 3) + 1) as i64
}

pub fn is_leap_year(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i64, m: i64) -> i64 {
    if m < 1 || m > 12 {
        return 0;
    }
    let next_month = if m == 12 {
        NaiveDate::from_ymd_opt(y as i32 + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(y as i32, m as u32 + 1, 1)
    };
    let first = NaiveDate::from_ymd_opt(y as i32, m as u32, 1);
    match (first, next_month) {
        (Some(a), Some(b)) => (b - a).num_days(),
        _ => 0,
    }
}

// ---------------- Aritmetica de fechas ----------------

pub fn add_seconds(ts: i64, n: i64) -> i64 {
    ts + n
}
pub fn add_minutes(ts: i64, n: i64) -> i64 {
    ts + n * 60
}
pub fn add_hours(ts: i64, n: i64) -> i64 {
    ts + n * 3600
}
pub fn add_days(ts: i64, n: i64) -> i64 {
    ts + n * 86400
}
pub fn add_weeks(ts: i64, n: i64) -> i64 {
    ts + n * 604800
}

/// Suma meses respetando fin de mes (Ej: 2024-01-31 + 1 mes = 2024-02-29).
pub fn add_months(ts: i64, n: i64) -> i64 {
    let dt = to_dt(ts);
    let mut y = dt.year();
    let mut m = dt.month() as i64;
    let d = dt.day();
    let h = dt.hour();
    let mi = dt.minute();
    let s = dt.second();
    m += n;
    while m > 12 {
        m -= 12;
        y += 1;
    }
    while m < 1 {
        m += 12;
        y -= 1;
    }
    let max_d = days_in_month(y as i64, m) as u32;
    let clamped = d.min(max_d);
    match NaiveDate::from_ymd_opt(y, m as u32, clamped).and_then(|nd| nd.and_hms_opt(h, mi, s)) {
        Some(ndt) => ndt.and_utc().timestamp(),
        None => ts,
    }
}

pub fn add_years(ts: i64, n: i64) -> i64 {
    add_months(ts, n * 12)
}

pub fn diff_seconds(a: i64, b: i64) -> i64 {
    a - b
}
pub fn diff_minutes(a: i64, b: i64) -> i64 {
    (a - b) / 60
}
pub fn diff_hours(a: i64, b: i64) -> i64 {
    (a - b) / 3600
}
pub fn diff_days(a: i64, b: i64) -> i64 {
    (a - b) / 86400
}

// ---------------- Comparaciones ----------------

pub fn is_before(a: i64, b: i64) -> bool {
    a < b
}
pub fn is_after(a: i64, b: i64) -> bool {
    a > b
}
pub fn is_same_day(a: i64, b: i64) -> bool {
    let da = to_dt(a).date_naive();
    let db = to_dt(b).date_naive();
    da == db
}

// ---------------- Timezones ----------------

pub fn to_timezone(ts: i64, tz_name: &str) -> Result<String, String> {
    let tz: Tz = tz_name
        .parse()
        .map_err(|e: chrono_tz::ParseError| e.to_string())?;
    let dt = to_dt(ts).with_timezone(&tz);
    Ok(dt.to_rfc3339())
}

pub fn timezone_offset_seconds(ts: i64, tz_name: &str) -> Result<i64, String> {
    let tz: Tz = tz_name
        .parse()
        .map_err(|e: chrono_tz::ParseError| e.to_string())?;
    let dt = to_dt(ts).with_timezone(&tz);
    Ok(dt.offset().fix().local_minus_utc() as i64)
}

/// Retorna una lista de nombres de timezones populares
/// (chrono-tz tiene ~600, filtramos los más usados en Latinoamérica y globalmente).
pub fn common_timezones() -> Vec<String> {
    vec![
        "UTC",
        "America/Caracas",
        "America/Argentina/Buenos_Aires",
        "America/Mexico_City",
        "America/Bogota",
        "America/Lima",
        "America/Santiago",
        "America/Sao_Paulo",
        "America/New_York",
        "America/Los_Angeles",
        "America/Chicago",
        "Europe/Madrid",
        "Europe/London",
        "Europe/Berlin",
        "Europe/Paris",
        "Asia/Tokyo",
        "Asia/Shanghai",
        "Asia/Kolkata",
        "Asia/Dubai",
        "Australia/Sydney",
        "Africa/Cairo",
        "Pacific/Auckland",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

// ---------------- Formato ISO / RFC ----------------

pub fn to_iso(ts: i64) -> String {
    to_dt(ts).to_rfc3339()
}

/// Parse ISO 8601 / RFC 3339 → segundos Unix.
pub fn from_iso(s: &str) -> Result<i64, String> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.timestamp())
        .map_err(|e| e.to_string())
}

// ---------------- Formato humano ----------------

/// "hace 3 minutos" / "en 2 horas" / "hace 5 dias" / "ahora"
pub fn humanize(ts: i64, now: i64) -> String {
    let delta = ts - now;
    let ago = delta < 0;
    let abs = delta.abs();
    let (n, unit) = if abs < 60 {
        (abs, "segundo")
    } else if abs < 3600 {
        (abs / 60, "minuto")
    } else if abs < 86400 {
        (abs / 3600, "hora")
    } else if abs < 604800 {
        (abs / 86400, "dia")
    } else if abs < 2592000 {
        (abs / 604800, "semana")
    } else if abs < 31536000 {
        (abs / 2592000, "mes")
    } else {
        (abs / 31536000, "año")
    };
    let plural = if n == 1 { "" } else { "s" };
    if abs < 5 {
        return "ahora".into();
    }
    if ago {
        format!("hace {n} {unit}{plural}")
    } else {
        format!("en {n} {unit}{plural}")
    }
}

// ---------------- Range / iteracion ----------------

/// Genera timestamps entre start y end con step segundos.
pub fn range(start: i64, end: i64, step: i64) -> Vec<i64> {
    if step <= 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut cur = start;
    while cur < end {
        out.push(cur);
        cur += step;
    }
    out
}

pub fn is_weekend(ts: i64) -> bool {
    let dow = day_of_week(ts);
    dow == 5 || dow == 6 // sabado o domingo
}

/// Dias laborales (lun-vie) entre dos fechas inclusive.
pub fn business_days_between(a: i64, b: i64) -> i64 {
    let (start, end) = if a <= b { (a, b) } else { (b, a) };
    let start_day = to_dt(start).date_naive();
    let end_day = to_dt(end).date_naive();
    let total_days = (end_day - start_day).num_days() + 1;
    if total_days <= 0 {
        return 0;
    }
    let mut count = 0i64;
    let mut cur = start_day;
    for _ in 0..total_days {
        let dow = cur.weekday().num_days_from_monday();
        if dow < 5 {
            count += 1;
        }
        cur = cur.succ_opt().unwrap_or(cur);
    }
    count
}

/// Proximo dia de la semana (0=lunes, 6=domingo) despues de ts.
/// Si ts YA es ese dia, retorna el proximo (7 dias despues).
pub fn next_weekday(ts: i64, target_dow: i64) -> i64 {
    let cur_dow = day_of_week(ts);
    let mut diff = target_dow - cur_dow;
    if diff <= 0 {
        diff += 7;
    }
    ts + diff * 86400
}

// ---------------- Construcción ----------------

pub fn from_ymd(y: i64, m: i64, d: i64) -> Result<i64, String> {
    NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32)
        .and_then(|nd| nd.and_hms_opt(0, 0, 0))
        .map(|ndt| ndt.and_utc().timestamp())
        .ok_or_else(|| format!("invalid date {y}-{m}-{d}"))
}

pub fn from_ymd_hms(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64) -> Result<i64, String> {
    NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32)
        .and_then(|nd| nd.and_hms_opt(h as u32, mi as u32, s as u32))
        .map(|ndt| ndt.and_utc().timestamp())
        .ok_or_else(|| format!("invalid datetime {y}-{m}-{d} {h}:{mi}:{s}"))
}

pub fn format(ts: i64, pattern: &str) -> String {
    to_dt(ts).format(pattern).to_string()
}

/// Parsea con format string custom (strftime-style).
pub fn parse(s: &str, pattern: &str) -> Result<i64, String> {
    NaiveDateTime::parse_from_str(s, pattern)
        .map(|dt| dt.and_utc().timestamp())
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_months_end_of_month() {
        // 2024-01-31 + 1 mes = 2024-02-29 (leap year)
        let jan31 = from_ymd(2024, 1, 31).unwrap();
        let feb = add_months(jan31, 1);
        assert_eq!(day(feb), 29);
        assert_eq!(month(feb), 2);
    }

    #[test]
    fn business_days_only_weekdays() {
        // Lunes 2024-01-01 → Domingo 2024-01-07 = 5 días laborales
        let lun = from_ymd(2024, 1, 1).unwrap();
        let dom = from_ymd(2024, 1, 7).unwrap();
        assert_eq!(business_days_between(lun, dom), 5);
    }

    #[test]
    fn humanize_relative() {
        let now = 1000000;
        assert!(humanize(now - 30, now).contains("segundo"));
        assert!(humanize(now + 3600, now).contains("hora"));
        assert!(humanize(now - 90000, now).contains("dia"));
    }

    #[test]
    fn timezone_conversion_venezuela() {
        // Verificación básica de que chrono-tz lee correctamente
        let ts = 1704067200; // 2024-01-01 00:00:00 UTC
        let ven = to_timezone(ts, "America/Caracas").unwrap();
        assert!(ven.starts_with("2023-12-31")); // -04:00
    }
}
