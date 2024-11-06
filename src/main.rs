use descord::prelude::*;
use lazy_static::lazy_static;

use std::collections::HashMap;
use tokio::sync::Mutex;

lazy_static! {
    static ref MAP: Mutex<HashMap<String, (String, usize)>> = Mutex::new(HashMap::new());
}

#[tokio::main]
async fn main() {
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
    client.register_slash_commands(vec![leaderboard()]).await;

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

    let mut binding = MAP.lock().await;
    let entry = binding.entry(author.id.clone()).or_default();
    entry.0 = author.username.clone();
    entry.1 += 1;
}

#[descord::slash]
async fn leaderboard(int: Interaction) {
    let mut map = MAP.lock().await.clone().into_iter().collect::<Vec<_>>();
    map.sort_unstable_by(|(_, (_, a)), (_, (_, b))| b.cmp(a));

    let mut embed = EmbedBuilder::new()
        .title("Message leaderboard")
        .color(Color::Orange);

    for (_user_id, (username, message_count)) in map.iter().take(10) {
        embed = embed.field(username, &message_count.to_string(), false);
    }

    int.reply(embed.build(), false).await;
}
