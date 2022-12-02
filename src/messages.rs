use chrono::{DateTime, Local, TimeZone};
use sqlx::MySqlPool;

use crate::{
    model::{
        api,
        db::{self, MessageRecord},
    },
    naive_to_local,
};

pub async fn get_messages(pool: &MySqlPool) -> anyhow::Result<Vec<MessageRecord>> {
    let messages = db::get_messages(pool).await?;
    Ok(messages)
}

pub async fn get_latest_message(pool: &MySqlPool) -> anyhow::Result<Option<MessageRecord>> {
    let message = db::get_latest_message(pool).await?;
    Ok(message)
}

async fn fetch_messages_as_match_as_possible_at_once<TzB, TzA>(
    pool: &MySqlPool,
    before: Option<&DateTime<TzB>>,
    after: Option<&DateTime<TzA>>,
    interval_ms: u64,
) -> anyhow::Result<Vec<MessageRecord>>
where
    TzB: TimeZone,
    TzB::Offset: std::fmt::Display,
    TzA: TimeZone,
    TzA::Offset: std::fmt::Display,
{
    let mut messages = Vec::new();
    let (limit, res_messages) = api::get_messages_with_time_section(0, before, after).await?;

    db::insert_messages(
        pool,
        &res_messages
            .iter()
            .map(MessageRecord::from)
            .collect::<Vec<MessageRecord>>(),
    )
    .await?;

    messages.extend(res_messages);

    let mut now = messages.len();

    while now < limit {
        let (_, res_messages) = api::get_messages_with_time_section(now, before, after).await?;

        let interval = tokio::spawn(async move {
            std::thread::sleep(std::time::Duration::from_micros(interval_ms));
        });

        db::insert_messages(
            pool,
            &res_messages
                .iter()
                .map(MessageRecord::from)
                .collect::<Vec<MessageRecord>>(),
        )
        .await?;

        messages.extend(res_messages);

        now = messages.len();

        interval.await.unwrap();
    }

    if messages.len() > limit {
        messages.truncate(limit);
    }

    Ok(messages.iter().map(MessageRecord::from).collect::<Vec<_>>())
}

pub async fn fetch_messages<Tz>(
    pool: &MySqlPool,
    limit: Option<usize>,
    after: Option<DateTime<Tz>>,
) -> anyhow::Result<Vec<MessageRecord>>
where
    Tz: TimeZone,
    Tz::Offset: std::fmt::Display,
{
    let mut messages = fetch_messages_as_match_as_possible_at_once(
        pool,
        None::<&DateTime<Local>>,
        after.as_ref(),
        300,
    )
    .await?;
    if messages.is_empty() {
        return Ok(messages);
    }
    loop {
        if let Some(limit) = limit {
            if messages.len() >= limit {
                break;
            }
        }
        let oldest_message = messages.last().unwrap();
        let oldest_message_id = &oldest_message.id;
        let oldest_message_created_at = oldest_message.created_at;
        let oldest_message_created_at_local = naive_to_local(oldest_message_created_at);
        let mut older_messages = fetch_messages_as_match_as_possible_at_once(
            pool,
            Some(&oldest_message_created_at_local),
            after.as_ref(),
            300,
        )
        .await?;

        // 現状保持しているメッセージの中で最も古いメッセージより新しいメッセージのみに絞る
        older_messages
            .retain(|m| m.id != *oldest_message_id && m.created_at >= oldest_message_created_at);

        if older_messages.is_empty() {
            break;
        }
        messages.extend(older_messages);
    }

    Ok(messages)
}
