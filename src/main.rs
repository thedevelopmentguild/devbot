use descord::prelude::*;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use nanoserde::{DeJson, SerJson};
use rand::Rng;
use redis::Commands;
use tokio::sync::Mutex;

const XP_INCREMENT_FACTOR: f32 = 1.3;
const INITIAL_XP: u32 = 100;

lazy_static! {
    pub static ref DB: Mutex<Option<redis::Connection>> = Mutex::new(None);
}

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
        "",
    )
    .await;

    client.register_events(vec![ready(), message_create()]);
    client.register_commands(vec![erase()]);
    client
        .register_slash_commands(vec![leaderboard(), rank()])
        .await;

    client.login().await;
}

#[descord::event]
async fn ready(data: ReadyData) {
    println!(
        "Logged in as: {}#{}",
        data.user.username, data.user.discriminator
    );
}

#[descord::event]
async fn message_create(msg: Message) {
    let author = msg.author.as_ref().unwrap();
    if author.bot {
        return;
    }

    let time: DateTime<Utc> = msg.timestamp.as_ref().unwrap().parse().unwrap();
    let epoch_time = time.timestamp();
    let username = author.username.clone();
    let user_id = author.id.clone();

    let userdata: Option<String> = db!()
        .hget(msg.guild_id.as_ref().unwrap(), &user_id)
        .unwrap();

    let xp = get_xp();

    // add user entry if it doesn't already exists
    if userdata.is_none() {
        let _: () = db!()
            .hset(
                msg.guild_id.as_ref().unwrap(),
                user_id.clone(),
                Data {
                    username,
                    user_id,
                    level: 0,
                    xp,
                    time: epoch_time as _,
                }
                .serialize_json(),
            )
            .unwrap();
    } else {
        let userdata = userdata.unwrap();
        let mut userdata = Data::deserialize_json(&userdata).unwrap();
        let current_time = chrono::Utc::now().timestamp();
        let last_message_time = userdata.time;

        // only give xp if a minute has passed since the last message
        if current_time - last_message_time as i64 > 60 {
            userdata.xp += xp;
            userdata.time = current_time as _;
            if userdata.xp > next_level_xp(userdata.level) {
                userdata.xp = 0;
                userdata.level += 1;
                msg.reply(format!(
                    "> You just reached level **{}**!\n > XP: 0/{}",
                    userdata.level,
                    next_level_xp(userdata.level),
                ))
                .await;
            }
        }

        let _: () = db!()
            .hset(
                msg.guild_id.as_ref().unwrap(),
                user_id.clone(),
                userdata.serialize_json(),
            )
            .unwrap();
    }
}

#[descord::slash(description = "View your (or someone else's) rank in this server.")]
async fn rank(int: Interaction, #[doc = "User to fetch avatar from"] user: Option<User>) {
    let user = user.as_ref().unwrap_or_else(|| int.user.as_ref().unwrap());

    let list: Vec<(String, String)> = db!().hgetall(&int.guild_id).unwrap_or_default();

    if list.is_empty() {
        int.reply("No messages yet :(", true).await;
        return;
    }

    let mut users = list
        .iter()
        .map(|(_, i)| Data::deserialize_json(i).unwrap())
        .collect::<Vec<_>>();

    users.sort_unstable_by(|a, b| b.level.cmp(&a.level));

    if let Some((rank, userdata)) = users
        .iter()
        .enumerate()
        .find(|(_, data)| data.user_id == user.id)
    {
        let embed = EmbedBuilder::new()
            .title(&format!("{}'s rank", int.user.as_ref().unwrap().username))
            .color(Color::Orange)
            .image(
                user.get_avatar_url(ImageFormat::WebP, None).unwrap(),
                None,
                None,
            )
            .description(&format!("Rank: #{}", rank + 1))
            .field("Level", &userdata.level.to_string(), true)
            .field(
                "XP",
                &format!("{}/{}", userdata.xp, next_level_xp(userdata.level)),
                true,
            )
            .build();

        int.reply(embed, false).await;
    } else {
        int.reply("You have 0 xp lol", true).await;
    }
}

#[descord::slash(description = "Displays the the list of two 10 users in this server.")]
async fn leaderboard(int: Interaction) {
    let list: Vec<(String, String)> = db!().hgetall(&int.guild_id).unwrap_or_default();

    if list.is_empty() {
        int.reply("No messages yet :(", true).await;
        return;
    }

    let mut users = list
        .iter()
        .map(|(_, i)| Data::deserialize_json(i).unwrap())
        .collect::<Vec<_>>();
    users.sort_unstable_by(|a, b| b.level.cmp(&a.level));

    let embed = EmbedBuilder::new()
        .color(Color::Cyan)
        .title(&utils::fetch_guild(&int.guild_id).await.unwrap().name)
        .fields(
            users
                .into_iter()
                .take(10)
                .enumerate()
                .map(
                    |(
                        rank,
                        Data {
                            username,
                            level,
                            xp,
                            ..
                        },
                    )| EmbedField {
                        name: format!("#{} - {username}", rank + 1),
                        value: format!("LVL: {level}, XP: {xp}"),
                        inline: false,
                    },
                )
                .collect(),
        )
        .build();

    int.reply(embed, false).await;
}

// FOR TESTING
#[descord::command(prefix = "!", permissions = "administrator")]
async fn erase(msg: Message) {
    let _: () = db!().del(msg.guild_id.as_ref().unwrap()).unwrap();
    msg.reply("Wiping database").await;
}

#[inline(always)]
fn get_xp() -> u32 {
    rand::thread_rng().gen_range(10..20)
}

#[inline(always)]
fn next_level_xp(current_level: u32) -> u32 {
    (INITIAL_XP as f32 * XP_INCREMENT_FACTOR.powi(current_level as _)) as _
}
