use actix_web::web;
use anyhow::Result;
use aws_sdk_s3::primitives::ByteStream;
use bollard::Docker;

use sea_orm::DbConn;
use tracing::info;

pub async fn init_db() -> Result<DbConn> {
    let database_url = std::env::var("DATABASE_URL")?;
    let db = sea_orm::Database::connect(&database_url).await?;
    db.ping().await?;
    info!("Database connected OK");
    Ok(db)
}

pub type WebDb = web::Data<DbConn>;

pub async fn init_docker() -> Result<Docker> {
    let docker = Docker::connect_with_defaults()?;
    let s = docker.ping().await?;
    info!("Docker connected {}", s);
    Ok(docker)
    // Ok(Docker::connect_with_defaults()?)
}

pub async fn init_rustfs() -> Result<aws_sdk_s3::Client> {
    let rustfs_endpoint_url = std::env::var("RUSTFS_ENDPOINT_URL")?;
    let rustfs_access_key_id = std::env::var("RUSTFS_ACCESS_KEY_ID")?;
    let rustfs_secret_access_key = std::env::var("RUSTFS_SECRET_ACCESS_KEY")?;
    let rustfs_region = std::env::var("RUSTFS_REGION")?;

    let creds = aws_sdk_s3::config::Credentials::new(
        rustfs_access_key_id,
        rustfs_secret_access_key,
        None,
        None,
        "floatctf",
    );

    let config = aws_sdk_s3::Config::builder()
        .region(aws_sdk_s3::config::Region::new(rustfs_region))
        .endpoint_url(rustfs_endpoint_url)
        .credentials_provider(creds)
        .force_path_style(true)
        .behavior_version(aws_config::BehaviorVersion::latest())
        .build();

    let client = aws_sdk_s3::Client::from_conf(config);
    info!("Rustfs connected OK");
    __init_buckets(&client).await?;

    Ok(client)
}

pub type WebDocker = web::Data<Docker>;
pub type WebRustfs = web::Data<aws_sdk_s3::Client>;

pub async fn __init_buckets(client: &aws_sdk_s3::Client) -> Result<()> {
    let floatctf_public_bucket_name = "floatctf-public";

    let floatctf_public_bucket = client
        .head_bucket()
        .bucket(floatctf_public_bucket_name)
        .send()
        .await;
    if floatctf_public_bucket.is_err() {
        client
            .create_bucket()
            .bucket(floatctf_public_bucket_name)
            .send()
            .await?;
        let policy = format!(
            r#"{{
                "Version": "2012-10-17",
                "Statement": [
                    {{
                        "Sid": "PublicReadGetObject",
                        "Effect": "Allow",
                        "Principal": "*",
                        "Action": ["s3:GetObject"],
                        "Resource": ["arn:aws:s3:::{}/*"]
                    }}
                ]
            }}"#,
            floatctf_public_bucket_name
        );

        client
            .put_bucket_policy()
            .bucket(floatctf_public_bucket_name)
            .policy(policy)
            .send()
            .await?;
        info!("Bucket {} created", floatctf_public_bucket_name);
    }

    // challenges 仍然保留，仅仅是将附件存入bucket challenges/name/attachement/file.zip
    let public_dirs = vec!["images/", "weapons/", "challenges/"];
    for dir in public_dirs {
        client
            .put_object()
            .bucket(floatctf_public_bucket_name)
            .key(dir)
            .body(ByteStream::from(vec![]))
            .send()
            .await?;
    }
    info!("Public dirs created");

    let floatctf_private_bucket_name = "floatctf-private";
    let floatctf_private_bucket = client
        .head_bucket()
        .bucket(floatctf_private_bucket_name)
        .send()
        .await;

    if floatctf_private_bucket.is_err() {
        client
            .create_bucket()
            .bucket(floatctf_private_bucket_name)
            .send()
            .await?;
        info!("Bucket {} created", floatctf_private_bucket_name);
    }

    let private_dirs = vec!["writeups"];

    for dir in private_dirs {
        client
            .put_object()
            .bucket(floatctf_private_bucket_name)
            .key(dir)
            .body(ByteStream::from(vec![]))
            .send()
            .await?;
    }
    info!("Private dirs created");

    Ok(())
}
