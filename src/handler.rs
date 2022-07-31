use log::{debug, error, info};
use rocket::futures::{channel::mpsc::UnboundedSender, SinkExt};
use tokio_tungstenite::tungstenite::Message;

use crate::{events::Events, generate_message, model::api};

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
                Events::DirectMessageCreated { channel_id }
                | Events::MessageCreated { channel_id } => {
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
            debug!("Received ping: {:?}", data);
        }
        _ => error!("Received: {:?} is not supported", message),
    };
}
