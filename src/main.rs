mod api;
mod events;

use std::{env, sync::Mutex, thread};

use dotenv::dotenv;
use events::Events;
use lindera::tokenizer::Tokenizer;
use markov::Chain;
use once_cell::sync::Lazy;
use rocket::futures::{StreamExt, SinkExt, future};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{handshake::client::generate_key, protocol::Message},
};

use log::{debug, error, info, warn};

pub static MARKOV_CHAIN: Lazy<Mutex<Chain<String>>> = Lazy::new(|| Mutex::new(Chain::of_order(3)));

pub const TARGET_USER_ID: &str = "81bbc211-65aa-4a45-8c56-e0b78d25f9e5";

pub static BOT_ACCESS_TOKEN: Lazy<String> = Lazy::new(|| {
    dotenv().ok();
    env::var("BOT_ACCESS_TOKEN").expect("BOT_ACCESS_TOKEN is not set")
});

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    env_logger::init();

    let ws_url = "wss://q.trap.jp/api/v3/bots/ws";
    // let ws_url = "ws://localhost:3000";

    match Chain::load(std::path::Path::new("tmp-sslime-markov")) {
        Ok(chain) => {
            *MARKOV_CHAIN.lock().unwrap() = chain;
        }
        Err(_) => feed_from_api(10000).await,
    }

    MARKOV_CHAIN
        .lock()
        .unwrap()
        .save("tmp-sslime-markov")
        .unwrap();

    let request = request_with_authorization(ws_url, BOT_ACCESS_TOKEN.as_str())?;

    let (ws_stream, _) = connect_async(request).await.unwrap();
    info!("Connected to {}", ws_url);

    let (tx, rx) = rocket::futures::channel::mpsc::unbounded();

    let (write, read) = ws_stream.split();

    let write_loop = rx.map(Ok).forward(write);


    let read_loop = {
        read.for_each(|message| async {
            let message = message.unwrap();
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
                        _ => error!("\"{:?}\" is not implemented", event),
                    }
                }
                Message::Ping(data) => {
                    debug!("Received ping: {:?}", data);
                    let msg = Message::Pong(data);
                    tx.clone().send(msg).await.unwrap();
                }
                _ => error!("Received: {:?} is not supported", message),
            };
        })
    };

    let _ = future::join(write_loop, read_loop).await;

    Ok(())
}

fn feed_message(message: &str) {
    let tokenizer = Tokenizer::new().unwrap();
    let tokens = tokenizer.tokenize_str(message).unwrap();
    let token = tokens.join(" ");
    MARKOV_CHAIN.lock().unwrap().feed_str(&token);
}

fn generate_message() -> String {
    MARKOV_CHAIN.lock().unwrap().generate().join("")
}

async fn feed_from_api(limit: usize) {
    let mut bar = progress::Bar::new();
    bar.set_job_title("Fetching messages");

    let mut messages = Vec::new();
    let r = api::get_messages(0).await;
    let (total_hit, contents) = r.unwrap();
    messages.extend(contents);
    let limit = limit.min(total_hit);
    let mut now = messages.len();
    while now < limit {
        let r = api::get_messages(now).await;
        messages.extend(r.unwrap().1);
        now = messages.len();
        bar.reach_percent((now * 100 / limit) as i32);
        thread::sleep(std::time::Duration::from_micros(500));
    }
    bar.jobs_done();

    let mut bar = progress::Bar::new();
    bar.set_job_title("Feeding messages");
    for (i, message) in messages.iter().enumerate() {
        feed_message(message);
        bar.reach_percent((i * 100 / messages.len()) as i32);
    }
    bar.jobs_done();
}

fn request_with_authorization(url: &str, token: &str) -> anyhow::Result<http::Request<()>> {
    let url = url::Url::parse(url)?;
    let host = url.host_str().unwrap();
    let req = http::Request::builder()
        .method("GET")
        .header("Host", host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-Websocket-Version", "13")
        .header("Sec-WebSocket-Key", generate_key())
        .uri(url.to_string())
        .header("Authorization", format!("Bearer {}", token))
        .body(())?;
    Ok(req)
}
