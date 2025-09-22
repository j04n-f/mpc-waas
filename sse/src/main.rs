mod config;

use std::collections::hash_map::{Entry, HashMap};
use std::sync::{
    Arc,
    atomic::{AtomicU16, Ordering},
};

use actix_web::Responder;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer, Result as ActixResult, middleware::Logger, web,
};
use actix_web_lab::sse::{self, Sse};
use futures_util::Stream;
use log::{debug, info};
use serde::{Deserialize, Serialize};
use tokio::sync::{Notify, RwLock};

use config::AppConfig;

async fn subscribe(
    db: web::Data<Db>,
    path: web::Path<String>,
    req: HttpRequest,
) -> ActixResult<HttpResponse> {
    let room_id = path.into_inner();
    let last_seen_msg = extract_last_event_id(&req);

    info!(
        "New subscription to room '{}' with last_seen_msg: {:?}",
        room_id, last_seen_msg
    );

    let room = db.get_room_or_create_for_index(&room_id).await;
    let subscribers = room.subscribers.load(Ordering::SeqCst);
    let subscription = room.subscribe(last_seen_msg);

    debug!(
        "Created subscription for room '{}', current subscribers: {}",
        room_id, subscribers
    );

    let stream = subscription_to_stream(subscription);

    Ok(Sse::from_stream(stream)
        .with_retry_duration(std::time::Duration::from_secs(5))
        .respond_to(&req))
}

async fn issue_idx(
    db: web::Data<Db>,
    path: web::Path<String>,
) -> ActixResult<web::Json<IssuedUniqueIdx>> {
    let room_id = path.into_inner();
    let room = db.get_room_or_create_for_index(&room_id).await;
    let idx = room.issue_unique_idx();

    info!("Issued unique index {} for room '{}'", idx, room_id);

    Ok(web::Json(IssuedUniqueIdx { unique_idx: idx }))
}

async fn broadcast(
    db: web::Data<Db>,
    path: web::Path<String>,
    message: String,
) -> ActixResult<HttpResponse> {
    let room_id = path.into_inner();
    let room = db.get_room_or_create_for_index(&room_id).await;

    debug!(
        "Broadcasting message to room '{}', message length: {} bytes",
        room_id,
        message.len()
    );

    room.publish(message).await;

    debug!("Message broadcast complete for room '{}'", room_id);

    Ok(HttpResponse::Ok().finish())
}

fn extract_last_event_id(req: &HttpRequest) -> Option<u16> {
    req.headers()
        .get("Last-Event-ID")
        .and_then(|header| header.to_str().ok())
        .and_then(|id_str| id_str.parse::<u16>().ok())
}

fn subscription_to_stream(
    mut subscription: Subscription,
) -> impl Stream<Item = Result<sse::Event, actix_web::Error>> {
    async_stream::stream! {
        loop {
            // Check if the client has disconnected by yielding a test event
            // If the client is gone, this will cause the stream to be dropped
            let (id, msg) = subscription.next().await;
            {
                let event = sse::Event::Data(
                    sse::Data::new(msg)
                        .event("new-message")
                        .id(id.to_string())
                );
                yield Ok(event);
            }
        }
    }
}

struct Db {
    rooms: RwLock<HashMap<String, Arc<Room>>>,
}

struct Room {
    messages: RwLock<Vec<String>>,
    message_appeared: Notify,
    subscribers: AtomicU16,
    next_idx: AtomicU16,
}

impl Db {
    pub fn empty() -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_room_or_create_for_index(&self, room_id: &str) -> Arc<Room> {
        let rooms = self.rooms.read().await;
        if let Some(room) = rooms.get(room_id) {
            debug!("Found existing room '{}'", room_id);
            return room.clone();
        }
        drop(rooms);

        let mut rooms = self.rooms.write().await;
        match rooms.entry(room_id.to_owned()) {
            Entry::Occupied(entry) => {
                debug!("Room '{}' was created by another thread", room_id);
                entry.get().clone()
            }
            Entry::Vacant(entry) => {
                info!("Creating new room '{}'", room_id);
                entry.insert(Arc::new(Room::empty())).clone()
            }
        }
    }
}

impl Room {
    pub fn empty() -> Self {
        Self {
            messages: RwLock::new(vec![]),
            message_appeared: Notify::new(),
            subscribers: AtomicU16::new(0),
            next_idx: AtomicU16::new(0),
        }
    }

    pub async fn publish(self: &Arc<Self>, message: String) {
        let mut messages = self.messages.write().await;
        let message_id = messages.len();
        messages.push(message);
        let subscriber_count = self.subscribers.load(Ordering::SeqCst);

        debug!(
            "Published message {} to {} subscribers",
            message_id, subscriber_count
        );

        self.message_appeared.notify_waiters();
    }

    pub fn subscribe(self: Arc<Self>, last_seen_msg: Option<u16>) -> Subscription {
        let new_count = self.subscribers.fetch_add(1, Ordering::SeqCst) + 1;
        let next_event = last_seen_msg.map(|i| i + 1).unwrap_or(0);

        debug!(
            "New subscription created, subscribers: {}, starting from event: {}",
            new_count, next_event
        );

        Subscription {
            room: self,
            next_event,
        }
    }

    pub fn issue_unique_idx(&self) -> u16 {
        self.next_idx.fetch_add(1, Ordering::Relaxed)
    }
}

struct Subscription {
    room: Arc<Room>,
    next_event: u16,
}

impl Subscription {
    pub async fn next(&mut self) -> (u16, String) {
        loop {
            let history = self.room.messages.read().await;
            if let Some(msg) = history.get(usize::from(self.next_event)) {
                let event_id = self.next_event;
                self.next_event = event_id + 1;
                debug!("Delivering event {} to subscriber", event_id);
                return (event_id, msg.clone());
            }
            debug!(
                "No new messages, waiting for notification (current event: {})",
                self.next_event
            );
            let notification = self.room.message_appeared.notified();
            drop(history);
            notification.await;
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        let remaining = self.room.subscribers.fetch_sub(1, Ordering::SeqCst) - 1;
        debug!("Subscription dropped, remaining subscribers: {}", remaining);

        if remaining == 0 {
            info!("Last subscriber left the room, room is now abandoned");
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct IssuedUniqueIdx {
    unique_idx: u16,
}

#[actix_web::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

    let app_config = AppConfig::from_env()?;

    let address = format!("{}:{}", app_config.sse.host, app_config.sse.port);

    info!("Starting SSE server at {address}",);

    let db = web::Data::new(Db::empty());

    HttpServer::new(move || {
        App::new()
            .app_data(db.clone())
            .app_data(
                web::PayloadConfig::new(100 * 1024 * 1024), // 100MB limit
            )
            .wrap(Logger::default())
            .route("/rooms/{room_id}/subscribe", web::get().to(subscribe))
            .route(
                "/rooms/{room_id}/issue_unique_idx",
                web::post().to(issue_idx),
            )
            .route("/rooms/{room_id}/broadcast", web::post().to(broadcast))
    })
    .bind(address)?
    .run()
    .await
    .map_err(anyhow::Error::from)
}
