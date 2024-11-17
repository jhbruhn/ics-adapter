use anyhow::Result;
use icalendar::{Calendar, Component, EventLike};
use serde::Serialize;
use std::collections::HashMap;
use warp::{Filter, Rejection, Reply};
use tracing_subscriber::fmt::format::FmtSpan;

async fn handler(params: HashMap<String, String>) -> Result<impl Reply, Rejection> {
    match params.get("url") {
        Some(url) => Ok(warp::reply::json(
            &convert(&[url], params.get("days"))
                .await
                .map_err(|_| warp::reject::reject())?
                .entries,
        )),
        None => Err(warp::reject::reject()),
    }
}

async fn new_handler(
    url: String,
    params: HashMap<String, String>,
) -> Result<impl Reply, Rejection> {
    let urls: Vec<&str> = url.split(";").collect();
    Ok(warp::reply::json(
        &convert(&urls, params.get("days"))
            .await
            .map_err(|_| warp::reject::reject())?
            .entries,
    ))
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
    calendar: u64,
}

async fn convert(urls: &[&str], days: Option<&String>) -> Result<CustomCalendar> {
    let mut entries = Vec::new();
    let mut calendar_index = 0;
    for url in urls {
        let url = urlencoding::decode(url)?.into_owned();
        tracing::info!("Converting for {}", url);
        let ics_text = reqwest::get(url).await?.text().await?;
        tracing::info!("Got text");

        let calendar = ics_text
            .parse::<Calendar>()
            .map_err(|e| anyhow::Error::msg(e))?;

        let filter_start = chrono::Utc::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        let filter_end =
            filter_start + chrono::Duration::days(days.and_then(|x| x.parse().ok()).unwrap_or(1));

        tracing::info!("Filtering for events between {} and {}", filter_start, filter_end);

        for event in calendar.components {
            
            if let Some(event) = event.as_event() {
                let Some(start) = event.get_start() else {
                    tracing::debug!("No start!");
                    continue;
                };

                let start = match convert_time(start) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::debug!("Invalid start timestamp: {:?}", e);
                        continue;
                    }
                };

                let end = match event.get_end() {
                    Some(end) => match convert_time(end) {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::debug!("Invalid end timestamp: {:?}", e);
                            continue;
                        }
                    },
                    None => start + chrono::Duration::days(1),
                };

                let length = end - start;

                let start_dates = if let Some(rrule) = event.properties().get("RRULE") {
                    //let vector = Vec::new();
                    let rrule_str = rrule.value();
                    let string = format!(
                        "DTSTART:{}\n{}",
                        event.properties().get("DTSTART").unwrap().value(),
                        rrule_str
                    );
                    let rrule: rrule::RRuleSet = string.parse()?;
                    let date_set = rrule
                        .into_iter()
                        .skip_while(|x| x < &filter_start)
                        .take_while(|x| x <= &filter_end)
                        .map(|x| x.with_timezone(&chrono::Utc))
                        .collect();
                    date_set
                } else {
                    vec![start]
                };

                tracing::debug!("Event times: {:?}", start_dates);

                for start in start_dates
                    .iter()
                    .skip_while(|x| x < &&filter_start)
                    .take_while(|x| x <= &&filter_end)
                {
                    tracing::debug!("Testing time {:?}", start);
                    let end = *start + length;

                    entries.push(CustomCalendarEntry {
                        title: event.get_summary().unwrap_or("").to_string(),
                        description: event.get_description().unwrap_or("").to_string(),
                        location: event.get_location().unwrap_or("").to_string(),
                        start: start.timestamp(),
                        end: end.timestamp(),
                        calendar: calendar_index,
                        isallday: start.time() == chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap()
                            && end - *start == chrono::Duration::days(1), // the event has a length of 24 hours and
                                                                          // starts at 00:00
                    });
                }
            }
        }

        calendar_index += 1;
    }

    entries.sort_by(|a, b| a.start.cmp(&b.start));

    tracing::info!("Returning {} entries", entries.len());

    Ok(CustomCalendar { entries })
}

fn convert_time(dt: icalendar::DatePerhapsTime) -> Result<chrono::DateTime<chrono::Utc>> {
    Ok(match dt {
        icalendar::DatePerhapsTime::DateTime(cdt) => {
            let cdt = match cdt {
                icalendar::CalendarDateTime::WithTimezone { date_time, tzid } => {
                    icalendar::CalendarDateTime::WithTimezone {
                        date_time,
                        tzid: String::from(match tzid.as_str() {
                            "Turkey Standard Time" => "Europe/Istanbul",
                            "India Standard Time" => "Asia/Kolkata",
                            "Pacific Standard Time" => "America/Los Angeles",
                            "W. Europe Standard Time" => "Europe/Berlin",
                            _ => &tzid,
                        }),
                    }
                }
                _ => cdt,
            };
            cdt.try_into_utc()
                .ok_or(anyhow::Error::msg("failed to convert to utc"))?
        }
        icalendar::DatePerhapsTime::Date(nd) => nd.and_hms_opt(0, 0, 0).unwrap().and_utc(),
    })
}

#[tokio::main]
async fn main() {
    let filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "ics_adapter=info,tracing=info,warp=debug".to_owned());
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(FmtSpan::CLOSE)
        .init();

    tracing::info!("Hello");

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
