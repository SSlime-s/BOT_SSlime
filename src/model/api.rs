use chrono::DateTime;
use log::debug;
use serde_json::Value;

use crate::{model::db::MessageRecord, BOT_ACCESS_TOKEN, BOT_ID, TARGET_USER_ID};

const BASE_URL: &str = "https://q.trap.jp/api/v3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    id: String,
    channel_id: String,
    content: String,
    created_at: String,
}
impl From<&Message> for MessageRecord {
    fn from(message: &Message) -> Self {
        MessageRecord {
            id: message.id.clone(),
            channel_id: message.channel_id.clone(),
            content: message.content.clone(),
            created_at: DateTime::parse_from_rfc3339(&message.created_at)
                .unwrap()
                .naive_local(),
        }
    }
}

fn create_client() -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    let authorization_token = format!("Bearer {}", *BOT_ACCESS_TOKEN);
    headers.insert(
        reqwest::header::AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(&authorization_token).unwrap(),
    );
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap()
}

/// /messages のレスポンスを解釈し、totalHits と messages の中身のタプルを返す
fn parse_messages_response(res: String) -> anyhow::Result<(usize, Vec<Message>)> {
    let res_json: Value = serde_json::from_str(&res)?;

    // schema: { "hits": { "content": "string" }[], "totalHits": number } }
    let hits = res_json["hits"].as_array().unwrap();
    let total_hits = res_json["totalHits"].as_u64().unwrap() as usize;
    let messages = hits
        .iter()
        .map(|hit| Message {
            id: hit["id"].as_str().unwrap().to_string(),
            channel_id: hit["channelId"].as_str().unwrap().to_string(),
            content: hit["content"].as_str().unwrap().to_string(),
            created_at: hit["createdAt"].as_str().unwrap().to_string(),
        })
        .collect::<Vec<Message>>();
    Ok((total_hits, messages))
}

/// /messages を offset に従って叩いて、totalHits と messages の中身のタプルを返す
pub async fn get_messages(offset: usize) -> anyhow::Result<(usize, Vec<Message>)> {
    let client = create_client();

    let url = format!("{BASE_URL}/messages");
    let res = client
        .get(&url)
        .query(&[
            ("word", ""),
            ("from", TARGET_USER_ID),
            ("limit", "100"),
            ("offset", &offset.to_string()),
            ("sort", "createdAt"),
        ])
        .send()
        .await?
        .text()
        .await?;

    parse_messages_response(res)
}

/// /messages を after と offset に従って叩いて、totalHits と messages の中身のタプルを返す
pub async fn get_messages_with_time_section<Tz>(
    offset: usize,
    after: &chrono::DateTime<Tz>,
) -> anyhow::Result<(usize, Vec<Message>)>
where
    Tz: chrono::TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let client = create_client();

    let url = format!("{}/messages", BASE_URL);
    let res = client
        .get(&url)
        .query(&[
            ("word", ""),
            ("from", TARGET_USER_ID),
            ("limit", "100"),
            ("offset", &offset.to_string()),
            ("sort", "createdAt"),
            ("after", &after.to_rfc3339()),
        ])
        .send()
        .await?
        .text()
        .await?;

    parse_messages_response(res)
}

/// 指定のチャンネルにメッセージを送信する
pub async fn post_message(channel_id: String, message: String) -> anyhow::Result<()> {
    let client = create_client();

    let url = format!("{}/channels/{}/messages", BASE_URL, channel_id);

    let request_body = serde_json::json!({
        "content": message,
        "embed": false,
    });

    let res = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(request_body.to_string())
        .send()
        .await?
        .text()
        .await?;

    debug!("{}", res);
    Ok(())
}

/// 指定のチャンネルに参加する
pub async fn join_channel(channel_id: String) -> anyhow::Result<()> {
    let client = create_client();

    let url = format!("{}/bots/{}/actions/join", BASE_URL, BOT_ID);

    let res = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(
            serde_json::json!({
                "channelId": channel_id,
            })
            .to_string(),
        )
        .send()
        .await?
        .text()
        .await?;

    debug!("{:?}", res);
    Ok(())
}

/// 指定のチャンネルから退出する
pub async fn leave_channel(channel_id: String) -> anyhow::Result<()> {
    let client = create_client();

    let url = format!("{}/bots/{}/actions/leave", BASE_URL, BOT_ID);

    let res = client
        .post(&url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(format!(r#"{{"channelId": "{}"}}"#, channel_id))
        .send()
        .await?
        .text()
        .await?;

    debug!("{}", res);
    Ok(())
}
