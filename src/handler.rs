use log::error;
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use sqlx::MySqlPool;
use traq_ws_bot::{events::payload, utils::is_mentioned_message};

use crate::{
    generate_message,
    model::{
        api,
        db::{get_frequency, update_frequency},
    },
    BOT_USER_ID, FREQUENCIES_CACHE, POOL,
};

const DEFAULT_FREQ: i64 = 20;

pub async fn join_handler(payload: payload::Joined) {
    let res = api::post_message(payload.channel.id, "参加しました :blob_pyon:".to_string()).await;
    if let Err(e) = res {
        error!("Failed to post message: {}", e);
    }
}

pub async fn left_handler(payload: payload::Left) {
    let res = api::post_message(
        payload.channel.id,
        "退出しました :blob_speedy_roll_inverse:".to_string(),
    )
    .await;
    if let Err(e) = res {
        error!("Failed to post message: {}", e);
    }
}

pub async fn direct_message_handler(payload: payload::DirectMessageCreated) {
    if payload.message.user.bot {
        return;
    }

    let res_message = generate_message();
    let res = api::post_message(payload.message.channel_id, res_message).await;
    if let Err(e) = res {
        error!("Failed to post message: {}", e);
    }
}

pub async fn non_mentioned_message_handler(payload: payload::MessageCreated) {
    if payload.message.user.bot {
        return;
    }

    if is_mentioned_message(&payload.message, BOT_USER_ID) {
        return;
    }

    let channel_id = payload.message.channel_id;
    let Some(freq) =
        get_frequency_with_cache(POOL.get().unwrap(), channel_id.clone()).await
    else {
        error!("Failed to get frequency");
        let res = api::post_message(
            channel_id,
            "頻度の取得に失敗しました :Hyperblob:".to_string(),
        ).await;
        if let Err(e) = res {
            error!("Failed to post message: {}", e);
        }
        return;
    };

    if freq < rand::thread_rng().gen_range(1..=100) {
        return;
    }

    let res_message = generate_message();
    let res = api::post_message(channel_id, res_message).await;
    if let Err(e) = res {
        error!("Failed to post message: {}", e);
    }
}

pub async fn mentioned_handler(payload: payload::MessageCreated) {
    if payload.message.user.bot {
        return;
    }

    if !is_mentioned_message(&payload.message, BOT_USER_ID) {
        return;
    }

    if payload.message.plain_text.contains("join") {
        let res = api::join_channel(payload.message.channel_id).await;
        if let Err(e) = res {
            error!("Failed to join channel: {}", e);
        }
        return;
    }

    if payload.message.plain_text.contains("leave") {
        let res = api::leave_channel(payload.message.channel_id).await;
        if let Err(e) = res {
            error!("Failed to leave channel: {}", e);
        }
        return;
    }

    if handle_try_change_freq(&payload.message).await {
        return;
    }

    let Some(freq) = get_frequency_with_cache(POOL.get().unwrap(), payload.message.channel_id.clone())
        .await
    else {
        error!("Failed to get frequency");
        let res = api::post_message(
            payload.message.channel_id,
            "頻度の取得に失敗しました :Hyperblob:".to_string(),
        ).await;
        if let Err(e) = res {
            error!("Failed to post message: {}", e);
        }
        return;
    };

    if freq < rand::thread_rng().gen_range(1..=100) {
        return;
    }
    let res_message = generate_message();
    let res = api::post_message(payload.message.channel_id, res_message).await;
    if let Err(e) = res {
        error!("Failed to post message: {}", e);
    }
}

static FREQ_COMMAND: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*@(?:\w|[_-])+\s+(?:\\|/)freq\s+(\S+)\s*$").unwrap());
pub async fn handle_try_change_freq(message: &traq_ws_bot::events::common::Message) -> bool {
    let Some(capture) = FREQ_COMMAND.captures(&message.plain_text) else {
        return false;
    };

    let freq = capture.get(1).unwrap().as_str();
    let res_msg;
    let changed_freq;

    match freq {
        "off" | "0" | "no" => {
            let res = update_frequency(POOL.get().unwrap(), message.channel_id.clone(), 0).await;
            match res {
                Ok(_) => {
                    changed_freq = Ok(0);
                    res_msg = "返答をしないように設定しました :blob_pyon:".to_string();
                }
                Err(e) => {
                    changed_freq = Err(());
                    res_msg = "頻度の更新に失敗しました :Hyperblob:".to_string();
                    error!("Failed to update frequency: {}", e);
                }
            }
        }
        "full" | "100" => {
            let res = update_frequency(POOL.get().unwrap(), message.channel_id.clone(), 100).await;
            match res {
                Ok(_) => {
                    changed_freq = Ok(100);
                    res_msg = "常に返答をするように設定しました :blob_pyon:".to_string();
                }
                Err(e) => {
                    changed_freq = Err(());
                    res_msg = "頻度の更新に失敗しました :Hyperblob:".to_string();
                    error!("Failed to update frequency: {}", e);
                }
            }
        }
        x if x.parse::<i64>().is_ok()
            && x.parse::<i64>().unwrap() > 0
            && x.parse::<i64>().unwrap() < 100 =>
        {
            let freq_int = x.parse::<i64>().unwrap();
            let res =
                update_frequency(POOL.get().unwrap(), message.channel_id.clone(), freq_int).await;
            match res {
                Ok(_) => {
                    changed_freq = Ok(freq_int);
                    res_msg = format!("頻度を {}% に設定しました :blob_pyon:", freq_int);
                }
                Err(e) => {
                    changed_freq = Err(());
                    res_msg = "頻度の更新に失敗しました :Hyperblob:".to_string();
                    error!("Failed to update frequency: {}", e);
                }
            }
        }
        x if x.parse::<i64>().is_ok() => {
            changed_freq = Err(());
            res_msg = "不正な数値です :Hyperblob: (0~100 expected)".to_string();
        }
        _ => {
            changed_freq = Err(());
            res_msg = "不正な引数です :Hyperblob: (0~100 expected)".to_string();
        }
    }
    if let Ok(freq) = changed_freq {
        FREQUENCIES_CACHE
            .lock()
            .unwrap()
            .insert(message.channel_id.clone(), freq);
    }
    let res = api::post_message(message.channel_id.clone(), res_msg).await;
    if let Err(e) = res {
        error!("Failed to post message: {}", e);
    }

    true
}

async fn get_frequency_with_cache(pool: &MySqlPool, channel_id: String) -> Option<i64> {
    let mut freq = FREQUENCIES_CACHE.lock().unwrap().get(&channel_id).copied();
    if freq.is_none() {
        freq = get_frequency(pool, channel_id.clone())
            .await
            .map(|x| x.map(|r| r.frequency).unwrap_or(DEFAULT_FREQ))
            .ok();
        if let Some(freq) = freq {
            FREQUENCIES_CACHE
                .lock()
                .unwrap()
                .insert(channel_id.clone(), freq);
        }
    }
    freq
}
