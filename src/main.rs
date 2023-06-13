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
        Some(url) => Ok(warp::reply::json(&convert(&url).await.map_err(|_| warp::reject::reject())?.entries)),
        None => Err(warp::reject::reject())
    }
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
}

async fn convert(url: &str) -> Result<CustomCalendar> {
    let ics_text = reqwest::get(url)
        .await?
        .text()
        .await?;
    
    let calendar = ics_text.parse::<Calendar>().map_err(|e| anyhow::Error::msg(e))?;

    let mut entries = Vec::new();
    
    let filter_start = now_timestamp_secs();
    let filter_end = now_timestamp_secs() + (24 * 60 * 60);

    for event in calendar.components {
        if let Some(event) = event.as_event() {
            let Some(start) = event.get_start() else {
                println!("No start!");
                continue;
            };
            let Some(end) = event.get_end() else {
                println!("No end!");
                continue;
            };

            let Ok(start) = convert_time(start) else {
                println!("Invalid timestamp");
                continue;
            };
            let Ok(end) = convert_time(end) else {
                println!("Invalid timestamp");
                continue;
            };

            if start < filter_start || start > filter_end {
                continue;
            }
            entries.push(CustomCalendarEntry {
                title: event.get_summary().unwrap_or("").to_string(),
                description: event.get_description().unwrap_or("").to_string(),
                location: event.get_location().unwrap_or("").to_string(),
                start,
                end
            });
        }
    }

    Ok(CustomCalendar{entries})
}

fn convert_time(dt: icalendar::DatePerhapsTime) -> Result<i64> {
    Ok(match dt {
        icalendar::DatePerhapsTime::DateTime(cdt) => cdt.try_into_utc().ok_or(anyhow::Error::msg("failed to convert to utc"))?.timestamp(),
        icalendar::DatePerhapsTime::Date(nd) => nd.and_hms_opt(0, 0, 0).unwrap().timestamp(),
    })
}

#[tokio::main]
async fn main() {
    let converter = warp::get()
        .and(warp::path("get"))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(handler);

    warp::serve(converter)
        .run(([0, 0, 0, 0], 3000))
        .await
}
