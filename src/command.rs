use super::*;

// User id of people who manage the bot
// thatmagicalcat. and duskyelf
const MANAGERS: [&str; 2] = ["815189874478546954", "820957214264000513"];

#[descord::command(description = "Reboot the bot")]
pub async fn reboot(msg: Message) {
    let userid = msg.author.as_ref().unwrap().id.as_str();
    if userid == MANAGERS[0] || userid == MANAGERS[1] {
        msg.reply("Restarting the bot").await;
        std::process::exit(0);
    }
}

#[descord::command(description = "Bot's latency")]
pub async fn ping(msg: Message) {
    let clock = std::time::Instant::now();
    let msg = msg.reply("Pong!").await;
    msg.edit(format!("Pong! `{}ms`", clock.elapsed().as_millis()))
        .await;
}

#[descord::command(
    prefix = "!",
    permissions = "manage_roles",
    description = "Create level roles"
)]
pub async fn setup_roles(msg: Message) {
    let guild_id = msg.guild_id.as_ref().unwrap().clone();
    if db!().hexists(&guild_id, "roles").unwrap() {
        msg.reply("Level roles already exists").await;
        return;
    }

    let clock = std::time::Instant::now();
    let initial = "Creating roles!";
    let msg = msg.reply(initial).await;

    let mut roles = Vec::with_capacity((NUM_ROLES / 5) as _);
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

#[descord::command(
    prefix = "!",
    permissions = "manage_roles",
    description = "Delete the level roles created in the server"
)]
pub async fn delete_roles(msg: Message) {
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

#[descord::command(description = "Assign level roles to people who don't have it.")]
pub async fn assign_roles(msg: Message) {
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
#[descord::command(prefix = "!", permissions = "administrator", description = "Wipe the enitre database :skull:")]
pub async fn erase(msg: Message) {
    msg.reply("wiping the database :skull:").await;
    let _: () = db!().del(msg.guild_id.as_ref().unwrap()).unwrap();
    msg.reply("dun :skull:").await;
}
