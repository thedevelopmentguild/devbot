use std::{cmp::Ordering, collections::HashMap};

use descord::prelude::*;

use chrono::{DateTime, Utc};
use lazy_static::lazy_static;
use nanoserde::{DeJson, SerJson};
use rand::Rng;
use redis::Commands;
use tokio::sync::Mutex;

const XP_INCREMENT_FACTOR: f32 = 1.3;
const INITIAL_XP: u32 = 100;
const NUM_ROLES: u32 = 50; // 50 / 5 = 10 roles

lazy_static! {
    pub static ref DB: Mutex<Option<redis::Connection>> = Mutex::new(None);
}

// "blazingly fast"
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

    client.register_events(vec![ready(), message_create()]);
    client.register_commands(vec![erase(), setup_roles(), delete_roles(), assign_roles()]);
    client
        .register_slash_commands(vec![leaderboard(), rank(), set_level()])
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

        if userdata.level == 0 && userdata.xp == 0 {
            // this is their first message
            assign_role(msg.guild_id.as_ref().unwrap(), &author.id, 0).await;
        }

        // only give xp if a minute has passed since the last message
        if current_time - last_message_time as i64 > 60 {
            userdata.xp += xp;
            userdata.time = current_time as _;
            if userdata.xp > next_level_xp(userdata.level) {
                userdata.xp = 0;
                userdata.level += 1;

                if userdata.level % 5 == 0 {
                    assign_role(msg.guild_id.as_ref().unwrap(), &author.id, userdata.level).await;
                }

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
    let user = user
        .as_ref()
        .unwrap_or_else(|| int.member.as_ref().unwrap().user.as_ref().unwrap());

    let list: Vec<(String, String)> = db!().hgetall(&int.guild_id).unwrap_or_default();

    if list.is_empty() {
        int.reply("No messages yet :(", true).await;
        return;
    }

    let mut users = list
        .iter()
        .map(|(_, i)| Data::deserialize_json(i))
        .filter_map(|i| i.ok())
        .collect::<Vec<_>>();

    users.sort_unstable_by(|a, b| match b.level.cmp(&a.level) {
        Ordering::Equal => b.xp.cmp(&a.xp),
        x => x,
    });

    if let Some((rank, userdata)) = users
        .iter()
        .enumerate()
        .find(|(_, data)| data.user_id == user.id)
    {
        let embed = EmbedBuilder::new()
            .title(&format!("{}'s rank", user.username))
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
        .map(|(_, i)| Data::deserialize_json(i)) // SAFETY: it will fail to parse
        .filter_map(|i| i.ok()) // that's why we need to filter
        .collect::<Vec<_>>();

    users.sort_unstable_by(|a, b| match b.level.cmp(&a.level) {
        Ordering::Equal => b.xp.cmp(&a.xp),
        x => x,
    });

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

#[descord::command(prefix = "!", permissions = "manage_roles")]
async fn setup_roles(msg: Message) {
    let guild_id = msg.guild_id.as_ref().unwrap().clone();
    if db!().hexists(&guild_id, "roles").unwrap() {
        msg.reply("Level roles already exists").await;
        return;
    }

    let clock = std::time::Instant::now();
    let initial = "Creating roles!";
    let msg = msg.reply(initial).await;

    let mut roles = Vec::with_capacity(8);
    for level in (0..=NUM_ROLES).step_by(5) {
        roles.push(format!("Level {level}"));
    }

    let mut roles_map: HashMap<u32, String> = HashMap::new();
    for (i, (level, role_name)) in roles
        .clone()
        .into_iter()
        .enumerate()
        .rev()
        .map(|(a, b)| (a * 5, b))
        .enumerate()
    {
        msg.edit(format!("{initial}\n{} of {}", i + 1, NUM_ROLES / 5 + 1))
            .await;

        roles_map.insert(
            level as _,
            utils::create_role(
                &guild_id,
                &role_name,
                0,
                Color::Rgb(100, 100, 100),
                false,
                false,
            )
            .await
            .unwrap()
            .id,
        );
    }

    msg.edit(&format!(
        "**I created the following roles:**\n```\n{}\n```\n-# Took {}s",
        roles.join("\n"),
        clock.elapsed().as_secs()
    ))
    .await;

    let _: () = db!()
        .hset(guild_id, "roles", roles_map.serialize_json())
        .unwrap();
}

#[descord::command(prefix = "!", permissions = "manage_roles")]
async fn delete_roles(msg: Message) {
    let Ok(Some(roles)): redis::RedisResult<Option<String>> =
        db!().hget(msg.guild_id.as_ref().unwrap(), "roles")
    else {
        msg.reply("No level roles are added in this server").await;
        return;
    };

    let roles: HashMap<u32, String> = HashMap::deserialize_json(&roles).unwrap();
    let num_roles = roles.len();

    let guild_id = msg.guild_id.clone().unwrap();
    for (_, role_id) in roles {
        let guild_id = guild_id.clone();
        tokio::spawn(async move {
            utils::delete_role(&guild_id, &role_id).await.unwrap();
        });
    }

    let _: () = db!().hdel(msg.guild_id.as_ref().unwrap(), "roles").unwrap();

    msg.reply(format!("deleted {} level roles", num_roles))
        .await;
}

#[descord::slash(description = "Set a user's level, sets xp to zero")]
async fn set_level(
    int: Interaction,
    #[doc = "User to assign the level"] user: User,
    #[doc = "What level to assign"] level: isize,
) {
    let current_time = chrono::Utc::now().timestamp();
    let _: () = db!()
        .hset(
            &int.guild_id,
            &user.id,
            Data {
                username: int
                    .member
                    .as_ref()
                    .unwrap()
                    .user
                    .as_ref()
                    .unwrap()
                    .username
                    .clone(),
                user_id: user.id.clone(),
                time: current_time as _,
                level: *level as _,
                xp: 0,
            }
            .serialize_json(),
        )
        .unwrap();

    int.reply("Done", false).await;
}

#[descord::command(description = "Assign level roles to people who don't have it.")]
async fn assign_roles(msg: Message) {
    let guild_id = msg.guild_id.clone().unwrap();
    let list: Vec<(String, String)> = db!().hgetall(&guild_id).unwrap_or_default();

    if list.is_empty() {
        msg.reply("The db is empty <:bruh:1301194882341408831>")
            .await;
        return;
    }

    let users = list
        .iter()
        .map(|(_, i)| Data::deserialize_json(i)) // SAFETY: it will fail to parse
        .filter_map(|i| i.ok()) // that's why we need to filter
        .collect::<Vec<_>>();

    for user in users {
        let guild_id = guild_id.clone();
        tokio::spawn(async move {
            for level in (0..=user.level).step_by(5) {
                assign_role(&guild_id, &user.user_id, level).await;
            }
        });
    }

    msg.reply("roles assigned").await;
}

// FOR TESTING
#[descord::command(prefix = "!", permissions = "administrator")]
async fn erase(msg: Message) {
    msg.reply("wiping the database :skull:").await;
    let _: () = db!().del(msg.guild_id.as_ref().unwrap()).unwrap();
    msg.reply("dun :skull:").await;
}

#[inline(always)]
fn get_xp() -> u32 {
    rand::thread_rng().gen_range(10..20)
}

#[inline(always)]
fn next_level_xp(current_level: u32) -> u32 {
    (INITIAL_XP as f32 * XP_INCREMENT_FACTOR.powi(current_level as _)) as _
}

async fn assign_role(guild_id: &str, user_id: &str, level: u32) {
    let roles: String = db!().hget(guild_id, "roles").unwrap();

    let roles_map = HashMap::<u32, String>::deserialize_json(&roles).unwrap();
    let role_id = roles_map
        .get(&level)
        .expect("I didn't create this many roles lol");

    utils::add_role(guild_id, user_id, role_id).await.unwrap();
}
