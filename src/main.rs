use std::time::{SystemTime, UNIX_EPOCH};
use warp::{
    Filter,
    Reply,
    Rejection,
};
use std::collections::HashMap;
use anyhow::Result;
use icalendar::{Component, Calendar, EventLike};
use serde::Serialize;

fn now_timestamp_secs() -> i64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    (since_the_epoch.as_millis() / 1000).try_into().unwrap()
}

async fn handler(params: HashMap<String, String>) -> Result<impl Reply, Rejection> {
    match params.get("url") {
        Some(url) => Ok(warp::reply::json(&convert(&url, params.get("days")).await.map_err(|_| warp::reject::reject())?.entries)),
        None => Err(warp::reject::reject())
    }
}

async fn new_handler(url: String, params: HashMap<String, String>) -> Result<impl Reply, Rejection> {
        Ok(warp::reply::json(&convert(&url, params.get("days")).await.map_err(|_| warp::reject::reject())?.entries))
}

#[derive(Serialize)]
struct CustomCalendar {
    entries: Vec<CustomCalendarEntry>,   
}

#[derive(Serialize)]
struct CustomCalendarEntry {
    title: String,
    start: i64,
    end: i64,
    location: String,
    description: String,
    isallday: bool,
}

async fn convert(url: &str, days: Option<&String>) -> Result<CustomCalendar> {
    let url = urlencoding::decode(url)?.into_owned();
    let ics_text = reqwest::get(url)
        .await?
        .text()
        .await?;
    
    let calendar = ics_text.parse::<Calendar>().map_err(|e| anyhow::Error::msg(e))?;

    let mut entries = Vec::new();
    
    let filter_start = now_timestamp_secs();
    let filter_end = now_timestamp_secs() + (24 * 60 * 60) * days.unwrap_or(&String::from("1")).parse().unwrap_or(1) as i64;

    for event in calendar.components {
        if let Some(event) = event.as_event() {
            let Some(start) = event.get_start() else {
                println!("No start!");
                continue;
            };

            let start = match convert_time(start) {
                Ok(t) => { t },
                Err(e) => {
                    println!("Invalid start timestamp: {:?}", e);
                    continue;
                }
            };

            let end = match event.get_end() {
                Some(end) => {
                    match convert_time(end) {
                        Ok(t) => { t },
                        Err(e) => {
                            println!("Invalid end timestamp: {:?}", e);
                            continue;
                        }
                    }
                },
                None => start + chrono::Duration::days(1),
            };

            if start.timestamp() < filter_start || start.timestamp() > filter_end {
                continue;
            }
            entries.push(CustomCalendarEntry {
                title: event.get_summary().unwrap_or("").to_string(),
                description: event.get_description().unwrap_or("").to_string(),
                location: event.get_location().unwrap_or("").to_string(),
                start: start.timestamp(),
                end: end.timestamp(),
                isallday: start.time() == chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap() && end - start == chrono::Duration::days(1) // the event has a length of 24 hours and
                                                        // starts at 00:00
            });
        }
    }

    Ok(CustomCalendar{entries})
}

fn convert_time(dt: icalendar::DatePerhapsTime) -> Result<chrono::DateTime<chrono::Utc>> {
    Ok(match dt {
        icalendar::DatePerhapsTime::DateTime(cdt) => {
            let cdt = match cdt {
                icalendar::CalendarDateTime::WithTimezone{date_time, tzid} => {
                    icalendar::CalendarDateTime::WithTimezone{date_time, tzid: String::from(match tzid.as_str() {
                        "W. Europe Standard Time" => "Europe/London",
                        _ => &tzid
                    })}
                },
                _ => cdt,
            };
            cdt.try_into_utc().ok_or(anyhow::Error::msg("failed to convert to utc"))?
        },
        icalendar::DatePerhapsTime::Date(nd) => nd.and_hms_opt(0, 0, 0).unwrap().and_utc(),
    })
}

#[tokio::main]
async fn main() {
    let converter = warp::path("get")
        .and(warp::query::<HashMap<String, String>>())
        .and_then(handler);

    let new_converter = warp::path!("calendar" / String / "entries")
        .and(warp::query::<HashMap<String, String>>())
        .and_then(new_handler);

    warp::serve(warp::get().and(converter.or(new_converter)))
        .run(([0, 0, 0, 0], 3000))
        .await
}
