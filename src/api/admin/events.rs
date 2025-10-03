use std::fs::File;
use std::io::Write;

use super::super::preclude::*;
use crate::api::admin::challenges::generate_safe_name;
use crate::api::admin::event_teams::{TeamMemberResult, TeamResult};
use crate::api::service::{ScoreboardItem, TrendItem};
use crate::config::get_setting;
use crate::entity::prelude::EventWriteup;
use crate::entity::sea_orm_active_enums::EventTeamMemberRole;
use crate::entity::{event_team_members, event_writeup};
use crate::{
    api::service::{__get_scoreboard, __get_trend, calculate_next_dynamic_score},
    auth::SuperAdminJwtGuard,
    entity::{
        event_challenge_solves, event_challenges, event_teams, event_users, events,
        prelude::{
            Challenges, EventChallengeSolves, EventChallenges, EventTeams, EventUsers, Events,
            Users,
        },
        sea_orm_active_enums::EventType,
        users,
    },
};
use chrono::NaiveDateTime;
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect};
use zip::write::FileOptions;
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateEventRequest {
    pub r#type: EventType,
    pub title: String,
    pub description: Option<String>,
    pub hidden: bool,
    pub allow_join: bool,
    pub rules: String,
    pub start_time: NaiveDateTime,
    pub end_time: NaiveDateTime,
}

#[post("")]
pub async fn create_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    cer: Json<CreateEventRequest>,
) -> UniResult<events::Model> {
    let cer = cer.into_inner();

    let new_event = events::ActiveModel {
        r#type: Set(cer.r#type),
        title: Set(cer.title),
        description: Set(cer.description),
        start_time: Set(cer.start_time),
        hidden: Set(cer.hidden),
        allow_join: Set(cer.allow_join),
        end_time: Set(cer.end_time),
        rules: Set(cer.rules),
        ..Default::default()
    };

    let event = new_event.insert(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}

type UpdateEventRequest = CreateEventRequest;
#[put("/{id}")]
pub async fn update_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    uer: Json<UpdateEventRequest>,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let uer = uer.into_inner();

    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let mut m_event = event.into_active_model();

    m_event.r#type = Set(uer.r#type);
    m_event.title = Set(uer.title);
    m_event.description = Set(uer.description);
    m_event.start_time = Set(uer.start_time);
    m_event.end_time = Set(uer.end_time);
    m_event.allow_join = Set(uer.allow_join);
    m_event.hidden = Set(uer.hidden);

    let event = m_event.update(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PatchEventRequest {
    pub r#type: Option<EventType>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub hidden: Option<bool>,
    pub allow_join: Option<bool>,
    pub rules: Option<String>,
    pub start_time: Option<NaiveDateTime>,
    pub end_time: Option<NaiveDateTime>,
}
#[patch("/{id}")]
pub async fn patch_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    per: Json<PatchEventRequest>,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let per = per.into_inner();
    dbg!(&per);
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;
    let mut m_event = event.into_active_model();

    per.r#type.map(|t| m_event.r#type = Set(t));

    per.title.map(|t| {
        m_event.title = Set(t);
    });

    per.description.map(|d| {
        m_event.description = Set(d.into());
    });

    per.start_time.map(|s| {
        m_event.start_time = Set(s);
    });

    per.end_time.map(|e| {
        m_event.end_time = Set(e);
    });

    per.hidden.map(|h| {
        m_event.hidden = Set(h);
    });

    per.allow_join.map(|a| {
        m_event.allow_join = Set(a);
    });

    per.rules.map(|r| {
        m_event.rules = Set(r.into());
    });
    let event = m_event.update(db.get_ref()).await?;

    UniResponse::ok(event.into()).into()
}
#[get("")]
pub async fn get_events(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    query_params: Query<QueryParams>,
) -> UniResult<Vec<events::Model>> {
    let mut query_params = query_params.0;

    let stmt = Events::find();

    if let (Some(limit), Some(page)) = (query_params.limit, query_params.page) {
        let paginator = stmt.paginate(db.get_ref(), limit);
        let items = paginator.fetch_page(page.saturating_sub(1)).await?;
        query_params.total = Some(paginator.num_items().await? as usize);

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    } else {
        let items = stmt.all(db.get_ref()).await?;
        query_params.total = Some(items.len());

        UniResponse::ok_meta(items.into(), query_params.into()).into()
    }
}

#[get("/{id}")]
pub async fn get_event(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    id: Path<Uuid>,
) -> UniResult<events::Model> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    UniResponse::ok(event.into()).into()
}

#[delete("/{id}")]
pub async fn delete_event(_user: SuperAdminJwtGuard, db: WebDb, id: Path<Uuid>) -> UniResult<u64> {
    let event = Events::find_by_id(*id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", id)))?;

    let r = event.delete(db.get_ref()).await?;

    UniResponse::ok(r.rows_affected.into()).into()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataEventChallenge {
    pub name: String,
    pub category: String,
    pub points: f64,
    pub solved_count: u64,
    pub solved_percent: f64,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DataEventChallengeSolve {
    pub user_nickname: String,
    pub challenge_name: String,
    pub challenge_category: String,
    pub created_at: NaiveDateTime,
    pub bonus_points: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DataPresent {
    pub event: events::Model,
    pub user_count: u64,
    pub team_count: u64,
    pub solved_recent_15: Vec<DataEventChallengeSolve>, // 谁 什么题 什么时间 多少分
    pub event_challenges: Vec<DataEventChallenge>,
    pub scoreboard_top10: Vec<ScoreboardItem>,
    pub trend: Vec<TrendItem>,
}
// 每道题 小方形卡片 名称 分类， 分数， 解题人数，解题百分比

#[get("/{event_id}/data")]
pub async fn get_data(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<DataPresent> {
    let event = Events::find_by_id(*event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!(" {} not exist", event_id)))?;

    let user_count = EventUsers::find()
        .filter(event_users::Column::EventId.eq(*event_id))
        .count(db.get_ref())
        .await?;

    let team_count = {
        if event.r#type == EventType::JeopardyTeam {
            EventTeams::find()
                .filter(event_teams::Column::EventId.eq(*event_id))
                .count(db.get_ref())
                .await?
        } else {
            0
        }
    };

    let solved_recent_15 = EventChallengeSolves::find()
        .filter(event_challenge_solves::Column::EventId.eq(*event_id))
        .order_by_desc(event_challenge_solves::Column::CreatedAt)
        .limit(15)
        .find_also_related(Users)
        .find_also_related(Challenges)
        .all(db.get_ref())
        .await?
        .into_iter()
        .map(|(solve, user, challenge)| DataEventChallengeSolve {
            user_nickname: user.map(|u| u.nickname).unwrap_or_default(),
            challenge_name: challenge.clone().map(|c| c.name).unwrap_or_default(),
            challenge_category: challenge.map(|c| c.category).unwrap_or_default(),
            created_at: solve.created_at,
            bonus_points: solve.bonus_points,
        })
        .collect::<Vec<_>>();
    // for all event's challenges
    let event_challenges = EventChallenges::find()
        .filter(event_challenges::Column::EventId.eq(*event_id))
        .find_also_related(Challenges)
        .all(db.get_ref())
        .await?;

    let mut data_event_challenges = Vec::new();
    for (event_challenge, challenge) in event_challenges {
        let solved_count = EventChallengeSolves::find()
            .filter(event_challenge_solves::Column::EventId.eq(*event_id))
            .filter(event_challenge_solves::Column::ChallengeId.eq(event_challenge.challenge_id))
            .count(db.get_ref())
            .await?;

        let solved_percent = {
            if event.r#type == EventType::JeopardyTeam {
                solved_count as f64 / team_count as f64
            } else {
                solved_count as f64 / user_count as f64
            }
        };

        let points = calculate_next_dynamic_score(&db, event_challenge.points, solved_count)
            .await
            .map_err(|e| {
                UniError::CustomError(format!("calculate_next_dynamic_score error: {}", e))
            })?;

        let dec = DataEventChallenge {
            name: challenge.clone().map(|c| c.name).unwrap_or_default(),
            category: challenge.map(|c| c.category).unwrap_or_default(),
            points: points,
            solved_count,
            solved_percent,
        };

        data_event_challenges.push(dec);
    }
    data_event_challenges.sort_by(|a, b| b.solved_count.cmp(&a.solved_count));

    let scoreboard = __get_scoreboard(db.clone(), *event_id)
        .await
        .map_err(|e| UniError::CustomError(format!("{}", e)))?;

    let trend_items = __get_trend(db, *event_id)
        .await
        .map_err(|e| UniError::CustomError(format!("{}", e)))?;

    let scoreboard_top10 = scoreboard.into_iter().take(10).collect::<Vec<_>>();

    let data_present = DataPresent {
        event: event,
        user_count,
        team_count,
        solved_recent_15,
        event_challenges: data_event_challenges,
        scoreboard_top10,
        trend: trend_items,
    };

    UniResponse::ok(data_present.into()).into()
}

#[get("/{event_id}/report")]
pub async fn get_report(
    _user: SuperAdminJwtGuard,
    db: WebDb,
    event_id: Path<Uuid>,
) -> UniResult<String> {
    let event_id = event_id.into_inner();
    let event = Events::find_by_id(event_id)
        .one(db.get_ref())
        .await?
        .ok_or(UniError::NotFound(format!("Event {} not exist", event_id)))?;
    let upload_dir = get_setting(&db, "UPLOAD_DIR")
        .await
        .map_err(|e| UniError::CustomError(format!("Failed to get upload dir setting: {}", e)))?;
    let target_zip = std::path::Path::new(&upload_dir).join(format!(
        "{}_{}.zip",
        generate_safe_name(&event.title),
        event_id
    ));

    let event_writeups = EventWriteup::find()
        .filter(event_writeup::Column::EventId.eq(event_id))
        .all(db.get_ref())
        .await?;

    let writeup_paths = event_writeups
        .iter()
        .map(|w| w.file_url.clone())
        .collect::<Vec<_>>();

    let zip_file = File::create(&target_zip).map_err(|e| UniError::CustomError(e.to_string()))?;
    let mut zip = zip::ZipWriter::new(zip_file);
    let options = FileOptions::<()>::default();

    zip.add_directory("uploads/", options)
        .map_err(|e| UniError::CustomError(e.to_string()))?;

    for wp_path in writeup_paths {
        let path = std::path::Path::new(&wp_path);
        if !path.exists() {
            // 可以选择忽略或者报错，这里我忽略并继续
            eprintln!("文件不存在: {:?}", path);
            continue;
        }
        let file_name = path
            .file_name()
            .ok_or(UniError::CustomError("无法获取文件名".to_string()))?;
        let file_name_str = file_name.to_str().ok_or(UniError::CustomError(
            "文件名不是有效的UTF-8字符串".to_string(),
        ))?;
        let file = File::open(path).map_err(|e| UniError::CustomError(e.to_string()))?;
        zip.start_file(
            format!("uploads/{}", file_name_str),
            FileOptions::<()>::default(),
        )
        .map_err(|e| UniError::CustomError(e.to_string()))?;
        std::io::copy(&mut &file, &mut zip).map_err(|e| UniError::CustomError(e.to_string()))?;
    }

    // 添加index.html报表 这里要根据不同的比赛类型设计
    let template_html = r#"
<html lang="zh-CN">
<head>
  <style>
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI",
        "Microsoft YaHei", "Helvetica Neue", Helvetica, Arial, sans-serif;
      line-height: 1.6;
      color: #333;
      max-width: 800px;
      margin: 20px auto;
      padding: 0 20px;
    }
    h1,
    h2,
    h3 {
      border-bottom: 1px solid #eaecef;
      padding-bottom: 0.3em;
    }
    h1 {
      font-size: 2em;
    }
    h2 {
      font-size: 1.5em;
    }
    h3 {
      font-size: 1.25em;
    }
    code {
      font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo,
        Courier, monospace;
      background-color: rgba(27, 31, 35, 0.05);
      padding: 0.2em 0.4em;
      font-size: 85%;
      border-radius: 3px;
    }
    table {
      width: 100%;   /* 跟随整个浏览器宽度 */
      max-width: 100%;
      border-collapse: collapse;
      margin-top: 1em;
    }
    th,
    td {
      border: 1px solid #ddd;
      padding: 0.6em;
      text-align: left;
    }
    thead {
      background-color: #f3f3f3;
    }
  </style>
  <meta http-equiv="Content-Type" content="text/html; charset=utf-8" />
  <meta charset="utf-8" />
  <title>{{ event.title }}' Writeup Report</title>
</head>
<body>
  <h1>{{ event.title }}' Writeup Report</h1>
  <p>Event ID：<code>{{ event.id }}</code></p>
  <p>Event Type：<code>{{ event.type }}</code></p>
  <p> Event Date：<code>{{ event.start_time }} - {{ event.end_time }}</code>
  </p> {% if event_teams_results %} <h2>Event Teams</h2>
  <table>
    <thead>
      <tr>
        <th>No.</th>
        <th>Team ID</th>
        <th>Name</th>
        <th>Points</th>
        <th>Member</th>
        <th>Writeup</th>
        <th>banned</th>
      </tr>
    </thead>
    <tbody> {% for team_result in event_teams_results %} <tr>
        <td>{{ loop.index }}</td>
        <td>{{ team_result.team.id}}</td>
        <td>{{ team_result.team.name }}</td>
        <td>{{ team_result.team.points }}</td>
        <td>
          <table>
            <thead>
              <tr>
                <th>Username</th>
                <th>Nickname</th>
                <th>Role</th>
                <th>Points</th>
              </tr>
            </thead>
            <tbody> {% for member in team_result.members%} <tr>
                <td>{{ member.username }}</td>
                <td>{{ member.nickname }}</td>
                <td>{{ member.role }}</td>
                <td>{{ member.points }}</td>
              </tr> {% endfor %} </tbody>
          </table>
        </td>
        <td><a href="{{ team_result.writeup_url }}" target="_blank">{{ team_result.writeup_url }}</a></td>
        <td>{{ team_result.team.banned }}</td>
      </tr> {% endfor %} </tbody>
  </table> {% endif %} {% if event_users %} <h2>Event Users</h2>
  <table>
    <thead>
      <tr>
        <th>No.</th>
        <th>Username</th>
        <th>Nickname</th>
        <th>Points</th>
        <th>Writeup</th>
        <th>Banned</th>
      </tr>
    </thead>
    <tbody> {% for user in event_users %} <tr>
        <td>{{ loop.index }}</td>
        <td>{{ user.username }}</td>
        <td>{{ user.nickname }}</td>
        <td>{{ user.points }}</td>
        <td><a href="{{ user.writeup_url }}" target="_blank">{{ user.writeup_url }}</a></td>
        <td>{{ user.banned }}</td>
      </tr> {% endfor %} </tbody>
  </table> {% endif %}
</html>
"#;
    let env = minijinja::Environment::new();
    let tmpl = env
        .template_from_str(template_html)
        .map_err(|e| UniError::CustomError(format!("Failed to create template: {}", e)))?;
    // prepare context

    let ctx = match event.r#type {
        EventType::JeopardySingle => {
            let event_users = EventUsers::find()
                .filter(event_users::Column::EventId.eq(event_id))
                .find_also_related(Users)
                .all(db.get_ref())
                .await?;
            let event_users_results = {
                let mut event_users_results = Vec::new();
                for (event_user, user) in event_users {
                    if let Some(user) = user {
                        let writeup = EventWriteup::find()
                            .filter(event_writeup::Column::UserId.eq(user.id))
                            .one(db.get_ref())
                            .await?;
                        let writeup_url = writeup.map(|w| w.file_url).unwrap_or_default();
                        let user_result = ReportUser {
                            username: user.username,
                            nickname: user.nickname,
                            points: event_user.points,
                            writeup_url,
                            banned: event_user.banned,
                        };
                        event_users_results.push(user_result);
                    }
                }
                event_users_results
            };
            minijinja::context! {
                event,
                event_users => event_users_results,
            }
        }

        EventType::JeopardyTeam => {
            let event_teams = EventTeams::find()
                .inner_join(EventWriteup) // with wp
                .filter(event_writeup::Column::EventId.eq(event_id))
                .all(db.get_ref())
                .await?;
            let event_teams_results = {
                let mut event_teams_results = Vec::new();
                for team in event_teams {
                    let members = team
                        .find_related(event_team_members::Entity)
                        .find_also_related(Users)
                        .all(db.get_ref())
                        .await?;
                    let mut team_members = Vec::new();

                    for (member, user) in members {
                        if let Some(user) = user {
                            let event_user = event_users::Entity::find()
                                .filter(event_users::Column::EventId.eq(event.id))
                                .filter(event_users::Column::UserId.eq(user.id))
                                .one(db.get_ref())
                                .await?
                                .ok_or(UniError::NotFound(format!(
                                    "EventUser {} not exist",
                                    user.id
                                )))?;

                            team_members.push(TeamMemberResult {
                                username: user.username,
                                nickname: user.nickname,
                                role: member.role,
                                points: event_user.points,
                            });
                        }
                    }

                    let writeup = EventWriteup::find()
                        .filter(event_writeup::Column::TeamId.eq(team.id))
                        .one(db.get_ref())
                        .await?;
                    let writeup_url = writeup.map(|w| w.file_url).unwrap_or_default();
                    let team_result = ReportTeam {
                        team,
                        writeup_url,
                        members: team_members,
                    };
                    event_teams_results.push(team_result);
                }
                event_teams_results
            };
            minijinja::context! {
                event,
                event_teams_results,
            }
        }
        _ => minijinja::context! {
            event,
        },
    };
    let rendered = tmpl
        .render(ctx)
        .map_err(|e| UniError::CustomError(format!("Failed to render template: {}", e)))?;
    zip.start_file("report.html", FileOptions::<()>::default())
        .map_err(|e| UniError::CustomError(e.to_string()))?;
    zip.write_all(rendered.as_bytes())
        .map_err(|e| UniError::CustomError(e.to_string()))?;
    // 返回zip文件的路径
    // uploads/c7b32b99-ed9e-476d-a7dc-b06b03e94c39.zip
    // writeups/
    // report.html
    // wp/1.pdf
    zip.finish()
        .map_err(|e| UniError::CustomError(e.to_string()))?;
    UniResponse::ok(target_zip.to_string_lossy().to_string().into()).into()
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ReportTeam {
    pub team: event_teams::Model,
    pub writeup_url: String,
    pub members: Vec<TeamMemberResult>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct ReportUser {
    pub username: String,
    pub nickname: String,
    pub points: f64,
    pub writeup_url: String,
    pub banned: bool,
}
