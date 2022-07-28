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
    FromBot,
}
impl Events {
    pub fn from_str(event_str: impl AsRef<str>) -> Result<Events, String> {
        let event_json: Value =
            serde_json::from_str(event_str.as_ref()).map_err(|e| e.to_string())?;
        let event_type = event_json["type"].as_str().unwrap();
        if event_json["body"]["user"]["bot"].as_bool().unwrap_or(false) {
            return Ok(Events::FromBot);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_events_ping() {
        let event_str = r#"{
            "type": "PING",
            "reqId": "476418eb-aed0-4300-9fe1-6fb397e3db16",
            "body": {
                "eventTime": "2019-05-07T04:50:48.582586882Z"
            }
        }"#;

        let event = Events::from_str(event_str).unwrap();
        assert_eq!(event, Events::Ping);
    }

    #[test]
    fn test_events_message_created() {
        let event_str = r#"{
            "type": "MESSAGE_CREATED",
            "reqId": "476418eb-aed0-4300-9fe1-6fb397e3db16",
            "body":{
                "eventTime": "2019-05-08T13:33:51.690308239Z",
                "message": {
                    "id": "bc9106b3-f9b2-4eca-9ba1-72b39b40954e",
                    "user": {
                    "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45",
                    "name": "takashi_trap",
                    "displayName": "寺田 健二",
                    "iconId": "2bc06cda-bdb9-4a68-8000-62f907f36a92",
                    "bot": false
                    },
                    "channelId": "9aba50da-f605-4cd0-a428-5e4558cb911e",
                    "text": "!{\"type\": \"user\", \"raw\": \"@takashi_trap\", \"id\": \"dfdff0c9-5de0-46ee-9721-2525e8bb3d45\"} こんにちは",
                    "plainText": "@takashi_trap こんにちは",
                    "embedded": [
                    {
                        "raw": "@takashi_trap",
                        "type": "user",
                        "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45"
                    }
                    ],
                    "createdAt": "2019-05-08T13:33:51.632149265Z",
                    "updatedAt": "2019-05-08T13:33:51.632149265Z"
                }
            }
        }"#;

        let event = Events::from_str(event_str).unwrap();
        assert_eq!(
            event,
            Events::MessageCreated {
                channel_id: "9aba50da-f605-4cd0-a428-5e4558cb911e".to_string()
            }
        );
    }

    fn test_events_direct_message_created() {
        let event_str = r#"{
            "type": "DIRECT_MESSAGE_CREATED",
            "reqId": "476418eb-aed0-4300-9fe1-6fb397e3db16",
            "body": {
                "eventTime": "2019-05-08T13:36:09.421492525Z",
                "message": {
                    "id": "2d7ff3f5-c313-4f4a-a9bb-0b5f84d2b6f8",
                    "user": {
                    "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45",
                    "name": "takashi_trap",
                    "displayName": "寺田 健二",
                    "iconId": "2bc06cda-bdb9-4a68-8000-62f907f36a92",
                    "bot": false
                    },
                    "channelId": "c5a5a697-3bad-4540-b2da-93dc88181d34",
                    "text": "!{\"type\": \"user\", \"raw\": \"@takashi_trap\", \"id\": \"dfdff0c9-5de0-46ee-9721-2525e8bb3d45\"} こんにちは",
                    "plainText": "@takashi_trap こんにちは",
                    "embedded": [
                    {
                        "raw": "@takashi_trap",
                        "type": "user",
                        "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45"
                    }
                    ],
                    "createdAt": "2019-05-08T13:36:09.365393261Z",
                    "updatedAt": "2019-05-08T13:36:09.365393261Z"
                }
            }
        }"#;

        let event = Events::from_str(event_str).unwrap();
        assert_eq!(
            event,
            Events::DirectMessageCreated {
                channel_id: "c5a5a697-3bad-4540-b2da-93dc88181d34".to_string()
            }
        );
    }

    fn test_events_mentioned() {
        let event_str = r#"{
            "type": "MESSAGE_CREATED",
            "reqId": "232b81f6-6818-4c2b-8ef2-3b90a0d00c7f",
            "body": {
                "eventTime": "2022-07-28T20:24:16.387269984Z",
                "message": {
                    "id": "5c5ae30b-45a8-4ef6-999b-3dae17ae7847",
                    "user": {
                        "id": "d3aea431-ba9f-46e1-b338-3032ce8e3d6c",
                        "name": "anko",
                        "displayName": "あんこ",
                        "iconId": "490aacf0-6e36-43e3-9e0e-d82f2c8cdcba",
                        "bot": false
                    },
                    "channelId": "265f0c67-dbe3-4009-8aa1-f1952866e023",
                    "text": "!{\"type\":\"user\",\"raw\":\"@BOT_SSlime\",\"id\":\"d8ff0b6c-431f-4476-9708-cb9d2e49b0a5\"} join",
                    "plainText": "@BOT_SSlime join",
                    "embedded": [
                        {
                            "raw": "@BOT_SSlime",
                            "type": "user",
                            "id": "d8ff0b6c-431f-4476-9708-cb9d2e49b0a5"
                        }
                    ],
                    "createdAt": "2022-07-28T20:24:16.381564Z",
                    "updatedAt": "2022-07-28T20:24:16.381564Z"
                }
            }
        }"#;

        let event = Events::from_str(event_str).unwrap();
        assert_eq!(
            event,
            Events::MentionMessageCreated {
                channel_id: "265f0c67-dbe3-4009-8aa1-f1952866e023".to_string(),
                content: "@BOT_SSlime join".to_string()
            }
        );
    }

    #[test]
    fn test_events_joined() {
        let event_str = r##"{
            "type": "JOINED",
            "reqId": "476418eb-aed0-4300-9fe1-6fb397e3db16",
            "body": {
            "eventTime": "2019-05-08T13:49:13.769110201Z",
                "channel": {
                    "id": "f86c925c-3002-4ba5-939a-c92344e534f9",
                    "name": "po",
                    "path": "#a/po",
                    "parentId": "ea452867-553b-4808-a14f-a47ee0009ee6",
                    "creator": {
                    "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45",
                    "name": "takashi_trap",
                    "displayName": "寺田 健二",
                    "iconId": "2bc06cda-bdb9-4a68-8000-62f907f36a92",
                    "bot": false
                    },
                    "createdAt": "2018-04-25T12:22:02Z",
                    "updatedAt": "2018-04-25T12:22:02Z"
                }
            }
        }"##;

        let event = Events::from_str(event_str).unwrap();
        assert_eq!(
            event,
            Events::Join {
                channel_id: "f86c925c-3002-4ba5-939a-c92344e534f9".to_string(),
            }
        );
    }

    #[test]
    fn test_events_left() {
        let event_str = r##"{
            "type": "LEFT",
            "reqId": "476418eb-aed0-4300-9fe1-6fb397e3db16",
            "body": {
            "eventTime": "2019-05-08T13:49:16.497848449Z",
                "channel": {
                    "id": "f86c925c-3002-4ba5-939a-c92344e534f9",
                    "name": "po",
                    "path": "#a/po",
                    "parentId": "ea452867-553b-4808-a14f-a47ee0009ee6",
                    "creator": {
                    "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45",
                    "name": "takashi_trap",
                    "displayName": "寺田 健二",
                    "iconId": "2bc06cda-bdb9-4a68-8000-62f907f36a92",
                    "bot": false
                    },
                    "createdAt": "2018-04-25T12:22:02Z",
                    "updatedAt": "2018-04-25T12:22:02Z"
                }
            }
        }"##;

        let event = Events::from_str(event_str).unwrap();
        assert_eq!(
            event,
            Events::Left {
                channel_id: "f86c925c-3002-4ba5-939a-c92344e534f9".to_string(),
            }
        );
    }

    fn test_events_from_bot() {
        let event_str = r#"{
            "type": "DIRECT_MESSAGE_CREATED",
            "reqId": "476418eb-aed0-4300-9fe1-6fb397e3db16",
            "body": {
                "eventTime": "2019-05-08T13:36:09.421492525Z",
                "message": {
                    "id": "2d7ff3f5-c313-4f4a-a9bb-0b5f84d2b6f8",
                    "user": {
                    "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45",
                    "name": "takashi_trap",
                    "displayName": "寺田 健二",
                    "iconId": "2bc06cda-bdb9-4a68-8000-62f907f36a92",
                    "bot": true
                    },
                    "channelId": "c5a5a697-3bad-4540-b2da-93dc88181d34",
                    "text": "!{\"type\": \"user\", \"raw\": \"@takashi_trap\", \"id\": \"dfdff0c9-5de0-46ee-9721-2525e8bb3d45\"} こんにちは",
                    "plainText": "@takashi_trap こんにちは",
                    "embedded": [
                    {
                        "raw": "@takashi_trap",
                        "type": "user",
                        "id": "dfdff0c9-5de0-46ee-9721-2525e8bb3d45"
                    }
                    ],
                    "createdAt": "2019-05-08T13:36:09.365393261Z",
                    "updatedAt": "2019-05-08T13:36:09.365393261Z"
                }
            }
        }"#;

        let event = Events::from_str(event_str).unwrap();
        assert_eq!(event, Events::FromBot,)
    }
}
