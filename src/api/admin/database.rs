use actix_web::rt::time::Instant;
use base64::Engine;
use sea_orm::sqlx::{self, Column, Row, TypeInfo, postgres::PgRow};
use serde_json::{Value, json};

use crate::{api::preclude::*, auth::SuperAdminJwtGuard};

#[derive(Debug, Serialize, Deserialize)]
pub struct SqlStatement {
    sql: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SqlResult {
    pub sql_type: String, // exec , query
    pub rows: Vec<Value>,
    pub count: usize,
    pub rows_affected: u64,
    pub elapsed_ms: u128,
}

#[post("/exec_sql")]
async fn exec_sql(
    db: WebDb,
    _user: SuperAdminJwtGuard,
    ss: Json<SqlStatement>,
) -> UniResult<SqlResult> {
    let sql = ss.into_inner().sql;

    let sql_command = |sql: &str| {
        sql.trim_start()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase()
    };
    let sql_cmd = sql_command(&sql);
    let start_time = Instant::now();
    match sql_cmd.as_str() {
        "select" | "show" | "describe" | "explain" | "with" => {
            // fetch_all 查询
            match sqlx::query(&sql)
                .fetch_all(db.get_postgres_connection_pool())
                .await
            {
                Ok(rows) => {
                    let elapsed = start_time.elapsed().as_millis();
                    let data = rows_to_json(rows);

                    let result = SqlResult {
                        sql_type: "query".to_string(),
                        rows: data.clone(),
                        count: data.len(),
                        rows_affected: 0,
                        elapsed_ms: elapsed,
                    };

                    return UniResponse::ok(result.into()).into();
                }
                Err(e) => return UniError::SQLError(e.to_string()).into(),
            }
        }
        _ => match sqlx::query(&sql)
            .execute(db.get_postgres_connection_pool())
            .await
        {
            Ok(res) => {
                return UniResponse::ok({
                    let elapsed = start_time.elapsed().as_millis();
                    SqlResult {
                        sql_type: "exec".to_string(),
                        rows: vec![],
                        count: 0,
                        rows_affected: res.rows_affected(),
                        elapsed_ms: elapsed,
                    }
                    .into()
                })
                .into();
            }
            Err(e) => return UniError::SQLError(e.to_string()).into(),
        },
    }
}

pub fn rows_to_json(rows: Vec<PgRow>) -> Vec<Value> {
    let mut out = Vec::new();
    for row in rows {
        let mut obj = serde_json::Map::new();
        for col in row.columns() {
            let name = col.name();
            let type_name = col.type_info().name(); // 获取 PostgreSQL 类型名

            let value = match type_name {
                // ✅ 字符串类型
                "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" | "CITEXT" => row
                    .try_get::<String, _>(name)
                    .map_or(Value::Null, |v| json!(v)),

                // ✅ 整数类型
                "INT2" | "INT4" | "INT8" | "OID" => row
                    .try_get::<i64, _>(name)
                    .map_or(Value::Null, |v| json!(v)),

                // ✅ 浮点类型
                "FLOAT4" | "FLOAT8" | "NUMERIC" => row
                    .try_get::<f64, _>(name)
                    .map_or(Value::Null, |v| json!(v)),

                // ✅ 布尔类型
                "BOOL" => row
                    .try_get::<bool, _>(name)
                    .map_or(Value::Null, |v| json!(v)),

                // ✅ UUID 类型
                "UUID" => row
                    .try_get::<uuid::Uuid, _>(name)
                    .map(|v| json!(v.to_string()))
                    .unwrap_or(Value::Null),

                // ✅ JSON / JSONB
                "JSON" | "JSONB" => row.try_get::<Value, _>(name).unwrap_or(Value::Null),

                // ✅ 时间类型
                "DATE" => row
                    .try_get::<chrono::NaiveDate, _>(name)
                    .map(|v| json!(v.to_string()))
                    .unwrap_or(Value::Null),
                "TIMESTAMP" | "TIMESTAMPTZ" => row
                    .try_get::<chrono::NaiveDateTime, _>(name)
                    .map(|v| json!(v.to_string()))
                    .unwrap_or(Value::Null),

                // ✅ BYTEA（二进制）转 base64
                "BYTEA" => row
                    .try_get::<Vec<u8>, _>(name)
                    .map(|v| json!(base64::engine::general_purpose::STANDARD.encode(v)))
                    .unwrap_or(Value::Null),

                // 其它全部转成字符串
                _ => row
                    .try_get::<String, _>(name)
                    .map_or(Value::Null, |v| json!(v)),
            };
            obj.insert(name.to_string(), json!(value));
        }
        out.push(Value::Object(obj));
    }
    out
}
