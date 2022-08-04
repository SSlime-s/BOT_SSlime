use std::env;

use dotenv::dotenv;
use log::{debug, error, info};
use once_cell::sync::Lazy;
use rand::Rng;
use regex::Regex;
use rocket::futures::{channel::mpsc::UnboundedSender, SinkExt};
use sqlx::MySqlPool;
use tokio_tungstenite::tungstenite::Message;

use crate::{
    events::Events,
    generate_message,
    model::{
        api,
        db::{get_frequency, update_frequency},
    },
    FREQUENCIES_CACHE, POOL,
};

const DEFAULT_FREQ: i64 = 20;

static OUTPUT_PING: Lazy<bool> = Lazy::new(|| {
    dotenv().ok();
    env::var("OUTPUT_PING")
        .ok()
        .map(|s| s == "1")
        .unwrap_or(false)
});

static FREQ_COMMAND: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^\s*@(?:\w|[_-])+\s+(?:\\|/)freq\s+(\S+)\s*$").unwrap());
pub async fn handler_message(message: Message, _tx: &UnboundedSender<Message>) {
    match message {
        Message::Text(text) => {
            info!("Received: {}", text);
            let event = match Events::from_str(&text) {
                Ok(event) => event,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
            match event {
                Events::Join { channel_id } => {
                    let res =
                        api::post_message(channel_id, "参加しました :blob_pyon:".to_string()).await;
                    match res {
                        Ok(_) => (),
                        Err(e) => error!("{}", e),
                    }
                }
                Events::Left { channel_id } => {
                    let res = api::post_message(
                        channel_id,
                        "退出しました :blob_speedy_roll_inverse:".to_string(),
                    )
                    .await;
                    match res {
                        Ok(_) => (),
                        Err(e) => error!("{}", e),
                    }
                }
                Events::DirectMessageCreated { channel_id } => {
                    let res_msg = generate_message();
                    let res = api::post_message(channel_id, res_msg).await;
                    match res {
                        Ok(_) => (),
                        Err(e) => {
                            error!("{}", e);
                        }
                    }
                }
                Events::MessageCreated { channel_id } => {
                    let freq =
                        get_frequency_with_cache(POOL.get().unwrap(), channel_id.clone()).await;
                    if freq.is_none() {
                        error!("Failed to get frequency");
                        let res = api::post_message(
                            channel_id.clone(),
                            "頻度の取得に失敗しました :Hyperblob:".to_string(),
                        )
                        .await;
                        if let Err(e) = res {
                            error!("{}", e);
                        }
                        return;
                    }
                    let freq = freq.unwrap();
                    if freq < rand::thread_rng().gen_range(1..=100) {
                        return;
                    }
                    let res_msg = generate_message();
                    let res = api::post_message(channel_id, res_msg).await;
                    match res {
                        Ok(_) => (),
                        Err(e) => {
                            error!("{}", e);
                        }
                    }
                }
                Events::MentionMessageCreated {
                    channel_id,
                    content,
                } => {
                    if content.contains("join") {
                        let res = api::join_channel(channel_id.clone()).await;
                        if let Err(e) = res {
                            error!("{}", e);
                        }
                        return;
                    } else if content.contains("leave") {
                        let res = api::leave_channel(channel_id.clone()).await;
                        if let Err(e) = res {
                            error!("{}", e);
                        }
                        return;
                    } else if let Some(capture) = FREQ_COMMAND.captures(&content) {
                        let freq = capture.get(1).unwrap().as_str();
                        let res_msg;
                        let mut changed_freq = None;
                        match freq {
                            "off" | "0" | "no" => {
                                let res =
                                    update_frequency(POOL.get().unwrap(), channel_id.clone(), 0)
                                        .await;
                                if let Err(e) = res {
                                    error!("{}", e);
                                    res_msg = "頻度の更新に失敗しました :Hyperblob:".to_string();
                                } else {
                                    changed_freq = Some(0);
                                    res_msg =
                                        "返答をしないように設定しました :blob_pyon:".to_string();
                                }
                            }
                            "full" | "100" => {
                                let res =
                                    update_frequency(POOL.get().unwrap(), channel_id.clone(), 100)
                                        .await;
                                if let Err(e) = res {
                                    error!("{}", e);
                                    res_msg = "頻度の更新に失敗しました :Hyperblob:".to_string();
                                } else {
                                    changed_freq = Some(100);
                                    res_msg =
                                        "常に返答をするように設定しました :blob_pyon:".to_string();
                                }
                            }
                            x if x.parse::<i64>().is_ok()
                                && 100 > x.parse::<i64>().unwrap()
                                && x.parse::<i64>().unwrap() > 0 =>
                            {
                                let freq_int = x.parse::<i64>().unwrap();
                                let res = update_frequency(
                                    POOL.get().unwrap(),
                                    channel_id.clone(),
                                    freq_int,
                                )
                                .await;
                                if let Err(e) = res {
                                    error!("{}", e);
                                    res_msg = "頻度の更新に失敗しました :Hyperblob:".to_string();
                                } else {
                                    changed_freq = Some(freq_int);
                                    res_msg =
                                        format!("頻度を {}% に設定しました :blob_pyon:", freq_int);
                                }
                            }
                            x if x.parse::<i64>().is_ok() => {
                                res_msg = "不正な数値です :Hyperblob: (0~100 expected)".to_string();
                            }
                            _ => {
                                res_msg = "不正な引数です :Hyperblob: (0~100 expected)".to_string();
                            }
                        }
                        if let Some(freq) = changed_freq {
                            FREQUENCIES_CACHE
                                .lock()
                                .unwrap()
                                .insert(channel_id.clone(), freq);
                        }
                        let res = api::post_message(channel_id, res_msg).await;
                        if let Err(e) = res {
                            error!("{}", e);
                        }
                        return;
                    }
                    let freq =
                        get_frequency_with_cache(POOL.get().unwrap(), channel_id.clone()).await;
                    if freq.is_none() {
                        error!("Failed to get frequency");
                        let res = api::post_message(
                            channel_id,
                            "頻度の取得に失敗しました :Hyperblob:".to_string(),
                        )
                        .await;
                        if let Err(e) = res {
                            error!("{}", e);
                        }
                        return;
                    }
                    let freq = freq.unwrap();
                    if freq < rand::thread_rng().gen_range(1..=100) {
                        return;
                    }
                    let res_msg = generate_message();
                    let res = api::post_message(channel_id, res_msg).await;
                    match res {
                        Ok(_) => (),
                        Err(e) => {
                            error!("{}", e);
                        }
                    }
                }
                Events::FromBot => {
                    info!("Received from bot (nop)");
                }
                _ => error!("\"{:?}\" is not implemented", event),
            }
        }
        Message::Ping(data) => {
            if *OUTPUT_PING {
                debug!("Received ping: {:?}", data);
            }
            debug!("Received ping: {:?}", data);
        }
        _ => error!("Received: {:?} is not supported", message),
    };
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
