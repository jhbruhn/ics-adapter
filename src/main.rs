use warp::{
    Filter,
    Reply,
    Rejection,
};
use std::collections::HashMap;
use anyhow::Result;
use icalendar::{Component, Calendar, EventLike};
use serde::Serialize;

async fn handler(params: HashMap<String, String>) -> Result<impl Reply, Rejection> {
    match params.get("url") {
        Some(url) => Ok(warp::reply::json(&convert(&[url], params.get("days")).await.map_err(|_| warp::reject::reject())?.entries)),
        None => Err(warp::reject::reject())
    }
}

async fn new_handler(url: String, params: HashMap<String, String>) -> Result<impl Reply, Rejection> {
    let urls: Vec<&str> = url.split(";").collect();
        Ok(warp::reply::json(&convert(&urls, params.get("days")).await.map_err(|_| warp::reject::reject())?.entries))
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

async fn convert(urls: &[&str], days: Option<&String>) -> Result<CustomCalendar> {
    let mut entries = Vec::new();
    for url in urls {
        let url = urlencoding::decode(url)?.into_owned();
        let ics_text = reqwest::get(url)
            .await?
            .text()
            .await?;
        
        let calendar = ics_text.parse::<Calendar>().map_err(|e| anyhow::Error::msg(e))?;

        
        let filter_start = chrono::Utc::now().date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();
        let filter_end = filter_start + chrono::Duration::days(days.unwrap_or(&String::from("1")).parse().unwrap_or(1));

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

                if start < filter_start || start > filter_end {
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
