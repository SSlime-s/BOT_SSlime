use std::{env, sync::Arc, thread, time::Duration};

use log::{debug, error};
use rand::Rng;
use sqlx::MySqlPool;
use tokio_cron_scheduler::{Job, JobScheduler};
use traq_ws_bot::utils::RateLimiter;

use crate::{generate_message, model::api, update_markov_chain};

pub async fn start_scheduling(
    pool: &'static MySqlPool,
    channel_id: &'static str,
    rate_limiter: Arc<RateLimiter>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    let main_scheduler = JobScheduler::new()?;

    dotenv::dotenv().ok();
    let many_msg = env::var("MANY_MSG").map(|s| s == "1").unwrap_or(false);
    let cron_schedule = if many_msg {
        "1/4 * * * * *"
    } else {
        "0 0,20,40 0-15,22-23 * * *"
    };

    // 日本時間で 0 0,20,40 0,7-23 * * * (cron は UTC)
    let post_job = Job::new_async(cron_schedule, move |_uuid, _lock| {
        let rate_limiter = rate_limiter.clone();
        Box::pin(async move {
            let next_span = rand::thread_rng().gen_range(1..20);
            debug!("scheduled at {} minutes later", next_span);
            if !many_msg {
                thread::sleep(Duration::from_secs(next_span * 60));
            }
            let message = generate_message();
            if let Err(e) =
                api::post_message(channel_id.to_string(), message, Some(&rate_limiter)).await
            {
                error!("{}", e);
            }
        })
    })?;

    let update_markov_job = Job::new_async("0 0 0 * * *", |_uuid, _lock| {
        Box::pin(async {
            let res = update_markov_chain(pool).await;
            if let Err(e) = res {
                error!("{}", e);
            }
        })
    })?;

    main_scheduler.add(post_job)?;
    main_scheduler.add(update_markov_job)?;

    Ok(main_scheduler.start()?)
}
