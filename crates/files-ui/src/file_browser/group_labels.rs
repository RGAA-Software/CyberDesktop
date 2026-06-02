use chrono::Datelike;
use chrono::NaiveDate;
use rust_i18n::t;

pub(super) fn localized_group_title(key: &str, fallback: &str) -> String {
    match key {
        "date:future" => t!("files.group.date.future").into_owned(),
        "date:today" => t!("files.group.date.today").into_owned(),
        "date:yesterday" => t!("files.group.date.yesterday").into_owned(),
        "date:earlier_week" => t!("files.group.date.earlier_week").into_owned(),
        "date:last_week" => t!("files.group.date.last_week").into_owned(),
        "date:earlier_month" => t!("files.group.date.earlier_month").into_owned(),
        "date:last_month" => t!("files.group.date.last_month").into_owned(),
        "date:earlier_year" => t!("files.group.date.earlier_year").into_owned(),
        "date:last_year" => t!("files.group.date.last_year").into_owned(),
        "date:unknown" => t!("files.group.date.unknown").into_owned(),
        "size:folder" => t!("files.group.size.folder").into_owned(),
        "size:empty" => t!("files.group.size.empty").into_owned(),
        "size:tiny" => t!("files.group.size.tiny").into_owned(),
        "size:small" => t!("files.group.size.small").into_owned(),
        "size:medium" => t!("files.group.size.medium").into_owned(),
        "size:large" => t!("files.group.size.large").into_owned(),
        "size:huge" => t!("files.group.size.huge").into_owned(),
        "type:folder" => t!("files.group.type.folder").into_owned(),
        "type:symlink" => t!("files.group.type.symlink").into_owned(),
        "type:other" => t!("files.group.type.other").into_owned(),
        "type:file" => t!("files.group.type.file").into_owned(),
        _ if key.starts_with("date:year:") => key["date:year:".len()..].to_string(),
        _ if key.starts_with("date:month:") => {
            format_month_year_key(&key["date:month:".len()..])
        }
        _ if key.starts_with("date:day:") => format_long_date_key(&key["date:day:".len()..]),
        _ if key.starts_with("type:ext:") => key["type:ext:".len()..].to_ascii_uppercase(),
        "tag:none" => t!("files.group.tag.untagged").into_owned(),
        _ if key.starts_with("tag:") => key["tag:".len()..].to_string(),
        _ => fallback.to_string(),
    }
}

fn format_month_year_key(payload: &str) -> String {
    let Some((year, month)) = parse_year_month(payload) else {
        return payload.to_string();
    };
    t!(
        "files.group.date.month_year",
        month = calendar_month(month),
        year = year
    )
    .into_owned()
}

fn format_long_date_key(payload: &str) -> String {
    let Some(date) = parse_ymd(payload) else {
        return payload.to_string();
    };
    t!(
        "files.group.date.long",
        weekday = calendar_weekday(date.weekday().num_days_from_sunday()),
        month = calendar_month(date.month()),
        day = date.day(),
        year = date.year()
    )
    .into_owned()
}

fn parse_year_month(payload: &str) -> Option<(i32, u32)> {
    if payload.len() != 6 {
        return None;
    }
    let year: i32 = payload[..4].parse().ok()?;
    let month: u32 = payload[4..].parse().ok()?;
    (month >= 1 && month <= 12).then_some((year, month))
}

fn parse_ymd(payload: &str) -> Option<NaiveDate> {
    if payload.len() != 8 {
        return None;
    }
    let year: i32 = payload[..4].parse().ok()?;
    let month: u32 = payload[4..6].parse().ok()?;
    let day: u32 = payload[6..].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn calendar_month(month: u32) -> String {
    match month {
        1 => t!("files.calendar.january").into_owned(),
        2 => t!("files.calendar.february").into_owned(),
        3 => t!("files.calendar.march").into_owned(),
        4 => t!("files.calendar.april").into_owned(),
        5 => t!("files.calendar.may").into_owned(),
        6 => t!("files.calendar.june").into_owned(),
        7 => t!("files.calendar.july").into_owned(),
        8 => t!("files.calendar.august").into_owned(),
        9 => t!("files.calendar.september").into_owned(),
        10 => t!("files.calendar.october").into_owned(),
        11 => t!("files.calendar.november").into_owned(),
        12 => t!("files.calendar.december").into_owned(),
        _ => month.to_string(),
    }
}

fn calendar_weekday(index: u32) -> String {
    match index {
        0 => t!("files.calendar.sunday").into_owned(),
        1 => t!("files.calendar.monday").into_owned(),
        2 => t!("files.calendar.tuesday").into_owned(),
        3 => t!("files.calendar.wednesday").into_owned(),
        4 => t!("files.calendar.thursday").into_owned(),
        5 => t!("files.calendar.friday").into_owned(),
        6 => t!("files.calendar.saturday").into_owned(),
        _ => index.to_string(),
    }
}
