mod cron;
mod events;
mod handler;
mod messages;
mod model;
mod utils;

use std::{env, sync::Mutex};

use chrono::{DateTime, Local, NaiveDateTime};
use dotenv::dotenv;
use lindera::tokenizer::Tokenizer;
use markov::Chain;
use once_cell::sync::{Lazy, OnceCell};
use regex::{Regex, RegexSet};
use rocket::futures::{future, StreamExt};
use sqlx::MySqlPool;
use tokio_tungstenite::{connect_async, tungstenite::handshake::client::generate_key};

use log::{debug, error, info};
use utils::{split_all_regex, SplittedElement};

use crate::{
    cron::start_scheduling,
    handler::handler_message,
    messages::{fetch_messages, get_latest_message, get_messages},
    model::db::{connect_db, get_markov_cache, update_markov_cache},
};

pub static MARKOV_CHAIN: Lazy<Mutex<Chain<String>>> = Lazy::new(|| Mutex::new(Chain::of_order(3)));

/// 収集するユーザーの UUID
pub const TARGET_USER_ID: &str = "81bbc211-65aa-4a45-8c56-e0b78d25f9e5";

/// この BOT の UUID
pub const BOT_ID: &str = "32bbdf6e-8170-4987-ba20-71ecc589e4a6";

/// この BOT の USER ID
pub const BOT_USER_ID: &str = "d8ff0b6c-431f-4476-9708-cb9d2e49b0a5";

/// 定期投稿するチャンネルの UUID
pub const CRON_CHANNEL_ID: &str = "11c32e27-5aa5-44f2-bc3b-ef8e94103ccf";

pub static BOT_ACCESS_TOKEN: Lazy<String> = Lazy::new(|| {
    dotenv().ok();
    env::var("BOT_ACCESS_TOKEN").expect("BOT_ACCESS_TOKEN is not set")
});

/// この正規表現に一致するメッセージは、markov chain に反映されない
pub static BLOCK_MESSAGE_REGEX: Lazy<RegexSet> = Lazy::new(|| {
    RegexSet::new(vec![
        r"^:awoo:$",
        r"^(?:https?:)?//(?:\w|[.-])+/\S+\n*$",
        r"^\[[^\[\]]+\]\((?:https?:)?//(?:\w|[.-])+/\S+\)\n*$",
        r"^う+\n*$",
    ])
    .unwrap()
});

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

/// format
/// !{"type":"user","raw":"@BOT_SSlime","id":"d8ff0b6c-431f-4476-9708-cb9d2e49b0a5"}
/// !{"type":"channel","raw":"#gps/times/SSlime/bot","id":"11c32e27-5aa5-44f2-bc3b-ef8e94103ccf"}
static SPECIAL_LINK_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"!\{"type":"\w+","raw":!"([^"]+)","id":"(?:\w|[-])+"\}"#).unwrap());

#[derive(Debug, Clone)]
enum ContentType {
    Text(String),
    Stamp(String),
    SpecialLink(String),
}
/// traQ のメッセージ用に一部特殊な単語を format する
/// 現在はスタンプと、メンションやチャンネルリンク を format し、1単語としてわけている
fn traq_message_format(messages: String) -> Vec<ContentType> {
    #[allow(unused_assignments)]
    let mut result = Vec::new();
    result = split_all_regex(messages, &SPECIAL_LINK_REGEX)
        .into_iter()
        .map(|elem| match elem {
            SplittedElement::Unmatched(text) => ContentType::Text(text),
            SplittedElement::Matched(matched) => ContentType::SpecialLink(matched),
        })
        .collect();
    result = result
        .into_iter()
        .flat_map(|content| match content {
            ContentType::Text(text) => split_all_regex(text, &STAMP_REGEX)
                .into_iter()
                .map(|elem| match elem {
                    SplittedElement::Unmatched(text) => ContentType::Text(text),
                    SplittedElement::Matched(matched) => ContentType::Stamp(matched),
                })
                .collect(),
            content => vec![content],
        })
        .collect();
    result
}

fn feed_messages(messages: &[String]) {
    let tokenizer = Tokenizer::new().unwrap();
    for message in messages {
        if BLOCK_MESSAGE_REGEX.is_match(message) {
            continue;
        }
        let message_elements = traq_message_format(message.to_string());
        let tokens = message_elements
            .iter()
            .flat_map(|e| match e {
                ContentType::Text(text) => tokenizer.tokenize_str(text).unwrap(),
                ContentType::Stamp(stamp) => vec![stamp.as_str()],
                ContentType::SpecialLink(link) => vec![link.as_str()],
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
                let after = get_latest_message(pool)
                    .await?
                    .map(|m| naive_to_local(m.created_at));
                let force_fetch = env::var("FORCE_FETCH").map(|v| v == "1").unwrap_or(false);
                fetch_messages(pool, None, if force_fetch { None } else { after }).await?;
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
