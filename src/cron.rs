use std::time::Duration;

use log::{error, debug};
use rand::Rng;
use sqlx::MySqlPool;
use tokio_cron_scheduler::{Job, JobScheduler};

use crate::{api, generate_message, update_markov_chain};

pub async fn start_scheduling(
    pool: &'static MySqlPool,
    channel_id: &'static str,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    let main_scheduler = JobScheduler::new()?;

    let create_post_job = Job::new_async("0 0/20/40 0,7-23 * * *", move |_uuid, _lock| {
        Box::pin(async {
            let next_span = rand::thread_rng().gen_range(1..20);
            let next_job =
                Job::new_one_shot_async(Duration::from_secs(next_span * 60), |_uuid, _lock| {
                    Box::pin(async {
                        let msg = generate_message();
                        let res = api::post_message(channel_id.to_string(), msg).await;
                        if let Err(e) = res {
                            error!("{}", e);
                        }
                    })
                });
            let next_job = match next_job {
                Ok(job) => job,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
            let post_scheduler = match JobScheduler::new() {
                Ok(scheduler) => scheduler,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
            if let Err(e) = (&post_scheduler).add(next_job) {
                error!("{}", e);
            }
            let post_loop = match post_scheduler.start() {
                Ok(loop_) => loop_,
                Err(e) => {
                    error!("{}", e);
                    return;
                }
            };
            debug!("scheduled at {} minutes later", next_span);
            if let Err(e) = post_loop.await {
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

    main_scheduler.add(create_post_job)?;
    main_scheduler.add(update_markov_job)?;

    Ok(main_scheduler.start()?)
}
