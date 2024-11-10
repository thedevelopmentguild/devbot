use std::{cmp::Ordering, collections::HashMap};

use descord::prelude::*;

use chrono::DateTime;
use chrono::Utc;
use lazy_static::lazy_static;
use nanoserde::{DeJson, SerJson};
use rand::Rng;
use redis::Commands;
use tokio::sync::Mutex;
use tokio::time::Duration;

mod command;
mod event;
mod slash_command;

const XP_INCREMENT_FACTOR: f32 = 1.123;
const INITIAL_XP: u32 = 100;
const NUM_ROLES: u32 = 50; // 50 / 5 = 10 roles
const VANISH_TIME: u32 = 5;

lazy_static! {
    pub static ref DB: Mutex<Option<redis::Connection>> = Mutex::new(None);
}

// "blazingly fast"
#[macro_export]
macro_rules! db {
    [] => { DB.lock().await.as_mut().unwrap() };
}

#[derive(DeJson, SerJson)]
struct Data {
    username: String,
    user_id: String,
    level: u32,
    xp: u32,
    time: usize,
}

#[tokio::main]
async fn main() {
    let client = redis::Client::open("redis://127.0.0.1/").expect("Failed to connect");
    *DB.lock().await = Some(client.get_connection().expect("db isn't running"));

    if dotenvy::dotenv().is_err() {
        eprintln!(".env file is not found");
    }

    env_logger::init();

    let mut client = Client::new(
        &std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not found"),
        GatewayIntent::ALL,
        "!",
    )
    .await;

    client.register_events(vec![event::ready(), event::message_create()]);
    client.register_commands(vec![
        command::reboot(),
        command::erase(),
        command::setup_roles(),
        command::delete_roles(),
        command::assign_roles(),
    ]);

    client
        .register_slash_commands(vec![
            slash_command::leaderboard(),
            slash_command::rank(),
            slash_command::set_level(),
        ])
        .await;

    client.login().await;
}

#[inline(always)]
pub fn get_xp() -> u32 {
    rand::thread_rng().gen_range(10..20)
}

#[inline(always)]
pub fn next_level_xp(current_level: u32) -> u32 {
    (INITIAL_XP as f32 * XP_INCREMENT_FACTOR.powi(current_level as _)) as _
}

pub async fn assign_role(guild_id: &str, user_id: &str, level: u32) {
    let roles: String = db!().hget(guild_id, "roles").unwrap();

    let roles_map = HashMap::<u32, String>::deserialize_json(&roles).unwrap();
    let role_id = roles_map
        .get(&level)
        .expect("I didn't create this many roles lol");

    utils::add_role(guild_id, user_id, role_id).await.unwrap();
}

pub async fn vanish(msg: Message) {
    tokio::time::sleep(Duration::from_secs(VANISH_TIME as _)).await;
    msg.delete().await;
}
