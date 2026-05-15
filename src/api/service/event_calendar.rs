use chrono::{DateTime, NaiveDateTime, Utc};
use quick_xml::Reader;
use quick_xml::events::Event;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{
    api::{
        prelude::*,
        util::{kv_get, kv_set},
    },
    entity::{events, sea_orm_active_enums::EventType},
    prelude::*,
};

// ── RSS fetch & parse ────────────────────────────────────────────────────────

const RSS_URL: &str = "https://ctftime.org/event/list/upcoming/rss/";
const CTFTIME_CACHE_KEY: &str = "ctftime_rss";
const CTFTIME_CACHE_TTL: i64 = 1800; // 30 minutes

/// CTFtime RSS 解析后的单条外部赛事
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CtftimeEvent {
    pub title: String,
    pub url: String,
    pub ctftime_url: String,
    pub start_date: String,
    pub finish_date: String,
    pub format_text: String,
    pub weight: String,
    pub restrictions: String,
    pub onsite: String,
    pub location: String,
    pub organizers: String,
    pub ctf_id: String,
}

/// 解析 CTFtime 的日期格式 YYYYMMDDTHHMMSS → UTC datetime
fn parse_ctftime_dt(dt_str: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(dt_str, "%Y%m%dT%H%M%S")
        .ok()
        .map(|naive| naive.and_utc())
}

/// 去除 HTML 标签
fn strip_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result.trim().to_string()
}

/// 发起 HTTP GET，抓取 RSS XML 原始字节
async fn fetch_rss(url: &str) -> Result<String, anyhow::Error> {
    let client = Client::builder()
        .user_agent(
            "Mozilla/5.0 (compatible; floatctf-rss/1.0; +https://github.com/0xm4k3/floatctf)",
        )
        .default_headers({
            let mut h = reqwest::header::HeaderMap::new();
            h.insert(
                reqwest::header::ACCEPT,
                reqwest::header::HeaderValue::from_static(
                    "application/rss+xml, application/xml, text/xml, */*",
                ),
            );
            h.insert(
                reqwest::header::ACCEPT_LANGUAGE,
                reqwest::header::HeaderValue::from_static("en-US,en;q=0.9"),
            );
            h
        })
        .timeout(std::time::Duration::from_secs(15))
        .build()?;

    let resp = client.get(url).send().await?;
    let body = resp.text().await?;
    Ok(body)
}

/// 解析 RSS XML，提取 item 列表
fn parse_rss(xml: &str) -> Result<Vec<CtftimeEvent>, quick_xml::Error> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut events = Vec::new();
    let mut current: Option<CtftimeEvent> = None;
    let mut current_tag = String::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "item" {
                    current = Some(CtftimeEvent {
                        title: String::new(),
                        url: String::new(),
                        ctftime_url: String::new(),
                        start_date: String::new(),
                        finish_date: String::new(),
                        format_text: String::new(),
                        weight: String::new(),
                        restrictions: String::new(),
                        onsite: String::new(),
                        location: String::new(),
                        organizers: String::new(),
                        ctf_id: String::new(),
                    });
                }
                current_tag = tag;
            }
            Ok(Event::Text(ref e)) => {
                if let Some(ref mut ev) = current {
                    let text = e.unescape().unwrap_or_default().to_string();
                    match current_tag.as_str() {
                        "title" => ev.title = text,
                        "url" => ev.url = text,
                        "ctftime_url" => ev.ctftime_url = text,
                        "start_date" => ev.start_date = text,
                        "finish_date" => ev.finish_date = text,
                        "format_text" => ev.format_text = text,
                        "weight" => ev.weight = text,
                        "restrictions" => ev.restrictions = text,
                        "onsite" => ev.onsite = text,
                        "location" => ev.location = text,
                        "organizers" => ev.organizers = text,
                        "ctf_id" => ev.ctf_id = text,
                        _ => {}
                    }
                }
            }
            Ok(Event::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "item" {
                    if let Some(ev) = current.take() {
                        events.push(ev);
                    }
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(e),
            _ => {}
        }
        buf.clear();
    }

    Ok(events)
}

// ── Calendar response types ──────────────────────────────────────────────────

/// 日历中统一的赛事条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub start_time: String,
    pub end_time: String,
    pub duration_hours: i64,
    pub format: String,
    pub location: String,
    pub onsite: bool,
    pub organizer: String,
    pub source: String, // "internal" | "ctftime"
    pub status: String, // "upcoming" | "running" | "ended"
}

/// 从缓存或远端获取 CTFtime RSS 解析结果
async fn get_ctftime_events(db: &sea_orm::DbConn) -> Vec<CtftimeEvent> {
    // 1. 尝试读缓存
    if let Ok(Some(cached)) = kv_get(db, CTFTIME_CACHE_KEY).await {
        if let Ok(events) = serde_json::from_value::<Vec<CtftimeEvent>>(cached) {
            tracing::debug!("CTFtime RSS: cache hit ({} events)", events.len());
            return events;
        }
    }

    // 2. 缓存未命中，实时拉取
    tracing::debug!("CTFtime RSS: cache miss, fetching...");
    match fetch_rss(RSS_URL).await {
        Ok(xml) => match parse_rss(&xml) {
            Ok(events) => {
                // 写入缓存
                if let Ok(json) = serde_json::to_value(&events) {
                    if let Err(e) =
                        kv_set(db, CTFTIME_CACHE_KEY, json, Some(CTFTIME_CACHE_TTL)).await
                    {
                        tracing::warn!("Failed to cache CTFtime RSS: {}", e);
                    }
                }
                events
            }
            Err(e) => {
                tracing::warn!("Failed to parse CTFtime RSS: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            tracing::warn!("Failed to fetch CTFtime RSS: {}", e);
            Vec::new()
        }
    }
}

/// GET /api/event_calendar
#[get("")]
pub async fn get_event_calendar(ctx: ReqCtx) -> UniResult<Vec<CalendarEvent>> {
    let mut all_events: Vec<CalendarEvent> = Vec::new();
    let now = Utc::now();

    // ── 1. 内部赛事 (events 表) ──────────────────────────────────────────
    let internal_events = events::Entity::find()
        .filter(events::Column::Hidden.eq(false))
        .order_by_desc(events::Column::StartTime)
        .all(ctx.db.get_ref())
        .await?;

    for ev in internal_events {
        let duration = (ev.end_time.timestamp() - ev.start_time.timestamp()) / 3600;
        let status = if now < ev.start_time {
            "upcoming"
        } else if now > ev.end_time {
            "ended"
        } else {
            "running"
        };

        let fmt = match ev.r#type {
            EventType::JeopardySingle => "Jeopardy (Single)",
            EventType::JeopardyTeam => "Jeopardy (Team)",
            EventType::JeopardyPractice => "Jeopardy (Practice)",
            EventType::AwdTeam => "AWD (Team)",
        };

        all_events.push(CalendarEvent {
            id: ev.id.to_string(),
            title: ev.title,
            description: ev.description,
            url: None,
            start_time: ev.start_time.to_rfc3339(),
            end_time: ev.end_time.to_rfc3339(),
            duration_hours: duration,
            format: fmt.to_string(),
            location: String::new(),
            onsite: false,
            organizer: String::new(),
            source: "internal".to_string(),
            status: status.to_string(),
        });
    }

    // ── 2. 外部赛事 (CTFtime RSS，带缓存) ────────────────────────────────
    for ev in get_ctftime_events(ctx.db.get_ref()).await {
        let start = parse_ctftime_dt(&ev.start_date);
        let finish = parse_ctftime_dt(&ev.finish_date);

        let (start_str, end_str, duration, status) = if let (Some(s), Some(f)) = (start, finish) {
            let dur = (f.timestamp() - s.timestamp()) / 3600;
            let status = if now < s {
                "upcoming"
            } else if now > f {
                "ended"
            } else {
                "running"
            };
            (s.to_rfc3339(), f.to_rfc3339(), dur, status)
        } else {
            (String::new(), String::new(), 0i64, "unknown")
        };

        let org_names = parse_organizers(&ev.organizers);

        all_events.push(CalendarEvent {
            id: format!("ctftime_{}", ev.ctf_id),
            title: strip_html(&ev.title),
            description: None,
            url: if ev.url.is_empty() {
                Some(format!("https://ctftime.org{}", ev.ctftime_url))
            } else {
                Some(ev.url.clone())
            },
            start_time: start_str,
            end_time: end_str,
            duration_hours: duration,
            format: ev.format_text,
            location: ev.location,
            onsite: ev.onsite == "True",
            organizer: org_names,
            source: "ctftime".to_string(),
            status: status.to_string(),
        });
    }

    // ── 3. 按 start_time 排序 ─────────────────────────────────────────────
    all_events.sort_by(|a, b| a.start_time.cmp(&b.start_time));

    UniResponse::ok(all_events.into()).into()
}

/// 解析 CTFtime organizers JSON: [{"name":"xxx"}, ...] → "xxx, yyy"
fn parse_organizers(raw: &str) -> String {
    #[derive(Deserialize)]
    struct Organizer {
        name: String,
    }

    serde_json::from_str::<Vec<Organizer>>(raw)
        .map(|orgs| {
            orgs.iter()
                .map(|o| o.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}
