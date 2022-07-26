mod api;
mod cron;
mod db;
mod events;
mod handler;
mod messages;

use std::{env, sync::Mutex};

use chrono::{DateTime, Local, NaiveDateTime};
use dotenv::dotenv;
use lindera::tokenizer::Tokenizer;
use markov::Chain;
use once_cell::sync::{Lazy, OnceCell};
use regex::Regex;
use rocket::futures::{future, StreamExt};
use sqlx::MySqlPool;
use tokio_tungstenite::{connect_async, tungstenite::handshake::client::generate_key};

use log::{debug, error, info};

use crate::{
    cron::start_scheduling,
    db::{connect_db, get_markov_cache, update_markov_cache},
    handler::handler_message,
    messages::{fetch_messages, get_latest_message, get_messages},
};

pub static MARKOV_CHAIN: Lazy<Mutex<Chain<String>>> = Lazy::new(|| Mutex::new(Chain::of_order(3)));

/// 収集するユーザーの UUID
pub const TARGET_USER_ID: &str = "81bbc211-65aa-4a45-8c56-e0b78d25f9e5";

/// この BOT の UUID
pub const BOT_ID: &str = "32bbdf6e-8170-4987-ba20-71ecc589e4a6";

/// この BOT の USER ID
pub const BOT_USER_ID: &str = "d8ff0b6c-431f-4476-9708-cb9d2e49b0a5";

/// 定期投稿するチャンネルの UUID
pub const CRON_CHANNEL_ID: &str = "todo";

pub static BOT_ACCESS_TOKEN: Lazy<String> = Lazy::new(|| {
    dotenv().ok();
    env::var("BOT_ACCESS_TOKEN").expect("BOT_ACCESS_TOKEN is not set")
});

/// この正規表現に一致するメッセージは、markov chain に反映されない
pub static BLOCK_MESSAGE_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^(?::awoo:|(?:https?:)//\S+\n*)?$").unwrap());

pub const SAVE_PATH: &str = "markov.yaml";

pub static POOL: OnceCell<MySqlPool> = OnceCell::new();

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    info!("Starting...");
    dotenv().ok();
    env_logger::init();

    let pool = connect_db().await?;
    POOL.set(pool).unwrap();

    debug!("db connected");

    let ws_url = "wss://q.trap.jp/api/v3/bots/ws";
    // let ws_url = "ws://localhost:3000";

    info!("loading markov chain cache...");
    update_markov_chain(POOL.get().unwrap()).await?;
    info!("markov chain loaded successfully !");

    info!("saving markov chain cache...");
    save_chain(POOL.get().unwrap()).await?;
    info!("markov chain saved successfully !");

    let request = request_with_authorization(ws_url, BOT_ACCESS_TOKEN.as_str())?;

    info!("connecting to {}...", ws_url);
    let (ws_stream, _) = connect_async(request).await.unwrap();
    info!("Connected to {}", ws_url);

    let (tx, rx) = rocket::futures::channel::mpsc::unbounded();

    let (write, read) = ws_stream.split();

    let write_loop = rx.map(Ok).forward(write);

    let read_loop = {
        read.for_each(|message| async {
            handler_message(message.unwrap(), &tx).await;
        })
    };

    let cron_loop = start_scheduling(POOL.get().unwrap(), CRON_CHANNEL_ID).await?;

    let _ = future::join3(write_loop, read_loop, cron_loop).await;

    Ok(())
}

static STAMP_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r":@?(?:\w|[-.])+:").unwrap());

#[derive(Debug, Clone)]
enum ContentType {
    Text(String),
    Stamp(String),
}
fn message_split_stamp(mut messages: String) -> Vec<ContentType> {
    let mut result = Vec::new();
    loop {
        let mat = STAMP_REGEX.find(&messages);
        match mat {
            Some(mat) => {
                if mat.start() == 0 {
                    let (stamp, rest) = messages.split_at(mat.end());
                    result.push(ContentType::Stamp(stamp.to_string()));
                    messages = rest.to_string();
                } else {
                    let (text, rest) = messages.split_at(mat.start());
                    result.push(ContentType::Text(text.to_string()));
                    let (stamp, rest) = rest.split_at(mat.end());
                    result.push(ContentType::Stamp(stamp.to_string()));
                    messages = rest.to_string();
                }
            }
            None => {
                if !messages.is_empty() {
                    result.push(ContentType::Text(messages));
                }
                break;
            }
        }
    }
    result
}

fn feed_messages(messages: &[String]) {
    let tokenizer = Tokenizer::new().unwrap();
    for message in messages {
        if BLOCK_MESSAGE_REGEX.is_match(message) {
            continue;
        }
        let message_elements = message_split_stamp(message.to_string());
        let tokens = message_elements
            .iter()
            .flat_map(|e| match e {
                ContentType::Text(text) => tokenizer.tokenize_str(text).unwrap(),
                ContentType::Stamp(stamp) => vec![stamp.as_str()],
            })
            .collect::<Vec<_>>();

        let token = tokens.join(" ");
        MARKOV_CHAIN.lock().unwrap().feed_str(&token);
    }
}

fn generate_message() -> String {
    MARKOV_CHAIN.lock().unwrap().generate().join("")
}

fn request_with_authorization(url: &str, token: &str) -> anyhow::Result<http::Request<()>> {
    let url = url::Url::parse(url)?;
    let host = url.host_str().unwrap();
    let req = http::Request::builder()
        .method("GET")
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-Websocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .uri(url.to_string())
        .header("Authorization", format!("Bearer {}", token))
        .body(())?;
    Ok(req)
}

pub async fn update_markov_chain(pool: &MySqlPool) -> anyhow::Result<()> {
    match load_chain(pool).await {
        Ok(res) => match res {
            Some(last_updated) => {
                if last_updated < Local::now().naive_local() - chrono::Duration::hours(20) {
                    debug!("markov chain is too old, updating...");
                    let messages =
                        fetch_messages(pool, None, Some(naive_to_local(last_updated))).await?;
                    feed_messages(
                        &messages
                            .iter()
                            .map(|m| m.content.clone())
                            .collect::<Vec<String>>(),
                    );
                }
            }
            None => {
                debug!("no cache found");
                let after = match get_latest_message(pool).await? {
                    Some(message) => Some(naive_to_local(message.created_at)),
                    None => None,
                };
                fetch_messages::<Local>(pool, None, after).await?;
                let messages = get_messages(pool).await?;
                feed_messages(
                    &messages
                        .iter()
                        .map(|m| m.content.clone())
                        .collect::<Vec<String>>(),
                );
            }
        },
        Err(e) => {
            error!("failed to load markov chain: {}", e);
            return Err(e);
        }
    };
    Ok(())
}

async fn save_chain(pool: &MySqlPool) -> anyhow::Result<()> {
    return Ok(());
    // DB への保存は showcase 上ではサイズの関係でうまく動かなかったから、実際には DB に保存しない
    {
        MARKOV_CHAIN.lock().unwrap().save(SAVE_PATH)?;
    }

    update_markov_cache(pool, "").await?;

    Ok(())
}

async fn load_chain(pool: &MySqlPool) -> anyhow::Result<Option<NaiveDateTime>> {
    return Ok(None);
    let content = get_markov_cache(pool).await?;

    let content = match content {
        Some(content) => content,
        None => return Ok(None),
    };

    match Chain::load(SAVE_PATH) {
        Ok(chain) => {
            *MARKOV_CHAIN.lock().unwrap() = chain;
        }
        Err(_e) => {
            return Ok(None);
        }
    }

    Ok(Some(content.last_update))
}

fn naive_to_local(naive: NaiveDateTime) -> DateTime<Local> {
    DateTime::<Local>::from_utc(naive, *Local::now().offset())
}
