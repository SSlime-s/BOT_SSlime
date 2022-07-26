use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Events {
    Ping,
    MessageCreated { channel_id: String },
    DirectMessageCreated { channel_id: String },
    MentionMessageCreated { channel_id: String, content: String },
}
impl Events {
    pub fn from_str(event_str: impl AsRef<str>) -> Result<Events, String> {
        let event_json: Value = serde_json::from_str(event_str.as_ref()).map_err(|e| e.to_string())?;
        let event_type = event_json["type"].as_str().unwrap();
        Ok(match event_type {
            "PING" => Events::Ping,
            "MESSAGE_CREATED" => {
                let channel_id = event_json["body"]["message"]["channelId"].as_str().ok_or("body.message.channelId is not string")?;
                Events::MessageCreated {
                    channel_id: channel_id.to_string(),
                }
            }
            "DIRECT_MESSAGE_CREATED" => {
                let channel_id = event_json["body"]["message"]["channelId"].as_str().ok_or("body.message.channelId is not string")?;
                Events::DirectMessageCreated {
                    channel_id: channel_id.to_string(),
                }
            }
            "MENTION_MESSAGE_CREATED" => {
                let channel_id = event_json["body"]["message"]["channelId"].as_str().ok_or("body.message.channelId is not string")?;
                let content = event_json["body"]["message"]["plainText"].as_str().ok_or("body.message.content is not string")?;
                Events::MentionMessageCreated {
                    channel_id: channel_id.to_string(),
                    content: content.to_string(),
                }
            }
            _ => return Err(format!("Unknown event type: {}", event_type)),
        })
    }
}
