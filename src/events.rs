use serde_json::Value;

use crate::BOT_USER_ID;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Events {
    Ping,
    Join { channel_id: String },
    Left { channel_id: String },
    MessageCreated { channel_id: String },
    DirectMessageCreated { channel_id: String },
    MentionMessageCreated { channel_id: String, content: String },
}
impl Events {
    pub fn from_str(event_str: impl AsRef<str>) -> Result<Events, String> {
        let event_json: Value =
            serde_json::from_str(event_str.as_ref()).map_err(|e| e.to_string())?;
        let event_type = event_json["type"].as_str().unwrap();
        Ok(match event_type {
            "PING" => Events::Ping,
            "MESSAGE_CREATED" => {
                let channel_id = event_json["body"]["message"]["channelId"]
                    .as_str()
                    .ok_or("body.message.channelId is not string")?;
                if event_json["body"]["message"]["embedded"]
                    .as_array()
                    .ok_or("body.message.embedded is not array")?
                    .iter()
                    .any(|emb| emb["id"].as_str() == Some(BOT_USER_ID))
                {
                    let content = event_json["body"]["message"]["plainText"]
                        .as_str()
                        .ok_or("body.message.plainText is not string")?;
                    Events::MentionMessageCreated {
                        channel_id: channel_id.to_string(),
                        content: content.to_string(),
                    }
                } else {
                    Events::MessageCreated {
                        channel_id: channel_id.to_string(),
                    }
                }
            }
            "DIRECT_MESSAGE_CREATED" => {
                let channel_id = event_json["body"]["message"]["channelId"]
                    .as_str()
                    .ok_or("body.message.channelId is not string")?;
                Events::DirectMessageCreated {
                    channel_id: channel_id.to_string(),
                }
            }
            "JOINED" => {
                let channel_id = event_json["body"]["channel"]["id"].as_str().unwrap();
                Events::Join {
                    channel_id: channel_id.to_string(),
                }
            }
            "LEFT" => {
                let channel_id = event_json["body"]["channel"]["id"].as_str().unwrap();
                Events::Left {
                    channel_id: channel_id.to_string(),
                }
            }
            _ => return Err(format!("Unknown event type: {}", event_type)),
        })
    }
}
