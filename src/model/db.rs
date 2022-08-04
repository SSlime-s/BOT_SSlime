use std::env;

use chrono::NaiveDateTime;
use dotenv::dotenv;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{FromRow, MySqlPool};

#[derive(Debug, FromRow)]
pub struct MarkovCacheRecord {
    pub cache: String,
    pub last_update: NaiveDateTime,
}

#[derive(Debug, FromRow)]
pub struct MessageRecord {
    pub id: String,
    pub channel_id: String,
    pub content: String,
    pub created_at: NaiveDateTime,
}

#[derive(Debug, FromRow)]
pub struct FrequencyRecord {
    pub channel_id: String,
    pub frequency: i64,
}

/// 環境変数を用いて、db に接続する
pub async fn connect_db() -> anyhow::Result<MySqlPool> {
    dotenv().ok();
    let hostname = env::var("MARIADB_HOSTNAME").unwrap();
    let database = env::var("MARIADB_DATABASE").unwrap();
    let username = env::var("MARIADB_USERNAME").unwrap();
    let password = env::var("MARIADB_PASSWORD").unwrap();

    let pool = MySqlPoolOptions::new()
        .max_connections(10)
        .connect(&format!(
            "mysql://{}:{}@{}/{}",
            username, password, hostname, database
        ))
        .await?;
    Ok(pool)
}

/// メッセージを保存する
pub async fn insert_messages(pool: &MySqlPool, messages: &[MessageRecord]) -> anyhow::Result<()> {
    if messages.is_empty() {
        return Ok(());
    }

    let query = format!(
        "INSERT IGNORE INTO messages (id, channel_id, content, created_at) VALUES {};",
        messages
            .iter()
            .map(|_| "(?, ?, ?, ?)")
            .collect::<Vec<_>>()
            .join(",")
    );

    let mut query = sqlx::query(&query);
    for message in messages {
        query = query.bind(&message.id);
        query = query.bind(&message.channel_id);
        query = query.bind(&message.content);
        query = query.bind(&message.created_at);
    }
    query.execute(pool).await?;

    Ok(())
}

/// メッセージを取得する
pub async fn get_messages(pool: &MySqlPool) -> anyhow::Result<Vec<MessageRecord>> {
    let messages: Vec<MessageRecord> = sqlx::query_as("SELECT * FROM messages;")
        .fetch_all(pool)
        .await?;
    Ok(messages)
}

pub async fn get_latest_message(pool: &MySqlPool) -> anyhow::Result<Option<MessageRecord>> {
    let message: Option<MessageRecord> =
        sqlx::query_as("SELECT * FROM messages ORDER BY created_at DESC LIMIT 1;")
            .fetch_optional(pool)
            .await?;
    Ok(message)
}

#[allow(dead_code)]
pub async fn get_frequencies(pool: &MySqlPool) -> anyhow::Result<Vec<FrequencyRecord>> {
    let frequencies: Vec<FrequencyRecord> = sqlx::query_as("SELECT * FROM `frequency`;")
        .fetch_all(pool)
        .await?;
    Ok(frequencies)
}

pub async fn get_frequency(
    pool: &MySqlPool,
    channel_id: String,
) -> anyhow::Result<Option<FrequencyRecord>> {
    let frequency: Option<FrequencyRecord> =
        sqlx::query_as("SELECT * FROM `frequency` WHERE `channel_id` = ?;")
            .bind(&channel_id)
            .fetch_optional(pool)
            .await?;
    Ok(frequency)
}

pub async fn update_frequency(
    pool: &MySqlPool,
    channel_id: String,
    frequency: i64,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO `frequency` (`channel_id`, `frequency`) VALUES (?, ?) ON DUPLICATE KEY UPDATE `frequency` = ?;")
        .bind(&channel_id)
        .bind(&frequency)
        .bind(&frequency)
        .execute(pool)
        .await?;
    Ok(())
}
