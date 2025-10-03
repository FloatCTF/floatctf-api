use crate::entity::sea_orm_active_enums::SettingValueType;
use crate::entity::settings;
use sea_orm::{
    ActiveValue::Set, ColumnTrait, DbConn, EntityTrait, QueryFilter, sea_query::OnConflict,
};

pub async fn init_settings(db: &DbConn) {
    let defaults = vec![
        (
            "INSTANCE_DESTROY_DELAY",
            "60",
            SettingValueType::Integer,
            "实例销毁延迟时间 (分钟)",
        ),
        (
            "EVENT_SCORE_DECAY",
            "15",
            SettingValueType::Integer,
            "比赛题目分数衰减系数",
        ),
        (
            "EVENT_SCORE_MIN_PERCENT",
            "0.45",
            SettingValueType::Float,
            "比赛题目最低分数为题目的百分比",
        ),
        (
            "CHALLENGES_DIR",
            "./fcmc/challenges",
            SettingValueType::String,
            "题目位置",
        ),
        (
            "HTTP_PREFIX",
            "http://",
            SettingValueType::String,
            "HTTP前缀",
        ),
        ("NODE_IP", "127.0.0.1", SettingValueType::String, "节点IP"),
        (
            "UPLOAD_DIR",
            "uploads",
            SettingValueType::String,
            "上传目录位置",
        ),
    ];

    for (key, value, value_type, description) in defaults {
        let e = settings::Entity::insert(settings::ActiveModel {
            key: Set(key.to_string()),
            value: Set(value.to_string()),
            r#type: Set(value_type),
            description: Set(description.to_string()),
            ..Default::default()
        })
        .on_conflict(
            OnConflict::column(settings::Column::Key)
                .do_nothing()
                .to_owned(),
        )
        .exec(db)
        .await;
        if let Err(err) = e {
            match err {
                sea_orm::DbErr::RecordNotInserted => {
                    tracing::debug!("Setting `{}` already exists, skipped", key);
                }
                _ => {
                    tracing::error!("Failed to insert setting `{}`: {}", key, err);
                }
            }
        }
    }
}

pub async fn get_setting(db: &DbConn, key: &str) -> Result<String, anyhow::Error> {
    settings::Entity::find()
        .filter(settings::Column::Key.eq(key))
        .one(db)
        .await
        .ok()
        .flatten()
        .map(|s| s.value)
        .ok_or(anyhow::anyhow!("Setting not found:{}", key))
}
