mod cron;
mod handler;
mod messages;
mod model;
mod utils;

use std::{
    collections::HashMap,
    env,
    sync::{Arc, Mutex},
    time::Duration,
};

use chrono::{DateTime, Local, NaiveDateTime};
use dotenv::dotenv;
use lindera::tokenizer::Tokenizer;
use markov::Chain;
use once_cell::sync::{Lazy, OnceCell};
use regex::{Regex, RegexSet};
use rocket::futures::future;
use sqlx::MySqlPool;

use log::{debug, info};
use traq_ws_bot::utils::RateLimiter;
use utils::{split_all_regex, SplittedElement};

use crate::{
    cron::start_scheduling,
    handler::{
        direct_message_handler, join_handler, left_handler, mentioned_handler,
        non_mentioned_message_handler,
    },
    messages::{fetch_messages, get_latest_message, get_messages},
    model::db::connect_db,
};

pub static MARKOV_CHAIN: Lazy<Mutex<Chain<String>>> = Lazy::new(|| Mutex::new(Chain::of_order(2)));

pub static FREQUENCIES_CACHE: Lazy<Mutex<HashMap<String, i64>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

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
        r"^%",
    ])
    .unwrap()
});

pub static POOL: OnceCell<MySqlPool> = OnceCell::new();

pub type Resource = Arc<Arc<RateLimiter>>;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();
    info!("Starting...");

    let pool = connect_db().await?;
    POOL.set(pool).unwrap();

    debug!("db connected");
    let rate_limiter = Arc::new(RateLimiter::new(5, Duration::from_secs(60)));

    let bot = traq_ws_bot::builder(&*BOT_ACCESS_TOKEN)
        .insert_resource(rate_limiter.clone())
        .on_joined(join_handler)
        .on_left(left_handler)
        .on_direct_message_created(direct_message_handler)
        .on_message_created_with_resource(non_mentioned_message_handler)
        .on_message_created_with_resource(mentioned_handler)
        .build();

    info!("loading markov chain cache...");
    update_markov_chain(POOL.get().unwrap()).await?;
    info!("markov chain loaded successfully !");

    let cron_loop = start_scheduling(POOL.get().unwrap(), CRON_CHANNEL_ID, rate_limiter).await?;

    let _ = future::join(bot.start(), cron_loop).await;

    Ok(())
}

static STAMP_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r":@?(?:\w|[-.])+:").unwrap());

/// format
///
/// `!{"type":"user","raw":"@BOT_SSlime","id":"d8ff0b6c-431f-4476-9708-cb9d2e49b0a5"}`
///
/// `!{"type":"channel","raw":"#gps/times/SSlime/bot","id":"11c32e27-5aa5-44f2-bc3b-ef8e94103ccf"}`
static SPECIAL_LINK_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"!\{"type":"\w+","raw":"([^"]+)","id":"(?:\w|[-])+"\}"#).unwrap());

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
            SplittedElement::Matched(matched) => ContentType::SpecialLink(
                SPECIAL_LINK_REGEX
                    .captures(&matched)
                    .unwrap()
                    .get(1)
                    .unwrap()
                    .as_str()
                    .to_string(),
            ),
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

pub async fn update_markov_chain(pool: &MySqlPool) -> anyhow::Result<()> {
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
    Ok(())
}

fn naive_to_local(naive: NaiveDateTime) -> DateTime<Local> {
    DateTime::<Local>::from_utc(naive, *Local::now().offset())
}
