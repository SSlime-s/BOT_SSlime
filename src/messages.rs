use chrono::{DateTime, TimeZone};
use sqlx::MySqlPool;

use crate::{
    api,
    db::{self, MessageRecord},
};

pub async fn get_messages(pool: &MySqlPool) -> anyhow::Result<Vec<MessageRecord>> {
    let messages = db::get_messages(pool).await?;
    Ok(messages)
}

pub async fn get_latest_message(pool: &MySqlPool) -> anyhow::Result<Option<MessageRecord>> {
    let message = db::get_latest_message(pool).await?;
    Ok(message)
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
    let mut messages = Vec::new();
    let r = match &after {
        Some(after) => api::get_messages_with_time_section(0, after).await?,
        None => api::get_messages(0).await?,
    };
    let (total_hit, res_messages) = r;

    db::insert_messages(
        pool,
        &res_messages
            .iter()
            .map(MessageRecord::from)
            .collect::<Vec<MessageRecord>>(),
    )
    .await?;

    messages.extend(res_messages);

    let limit = limit.map(|l| l.min(total_hit)).unwrap_or(total_hit);
    let mut now = messages.len();

    while now < limit {
        let r = match &after {
            Some(after) => api::get_messages_with_time_section(now, after).await?,
            None => api::get_messages(now).await?,
        };

        let interval = tokio::spawn(async {
            std::thread::sleep(std::time::Duration::from_micros(300));
        });

        let (_, res_messages) = r;

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

    let messages = messages.iter().map(MessageRecord::from).collect::<Vec<_>>();

    Ok(messages)
}
