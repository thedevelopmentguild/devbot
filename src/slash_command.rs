use super::*;

#[descord::slash(description = "Kick people below a certain level")]
pub async fn kick(int: Interaction, #[doc = "The min level required"] level: isize) {
    if *level < 0 {
        int.reply("Negative level?!?!", true).await;
        return;
    }

    let required_level = *level as u32;

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

    users.sort_unstable_by(|a, b| match a.level.cmp(&b.level) {
        Ordering::Equal => a.xp.cmp(&b.xp),
        x => x,
    });

    let users_below_min_level = users
        .iter()
        .filter(|user| user.level < required_level)
        .collect::<Vec<_>>();

    let embed = EmbedBuilder::new()
        .title(&format!("Members below {required_level} level"))
        .color(Color::Red)
        .description(
            &users_below_min_level
                .into_iter()
                .map(|i| format!("<@{}>", i.user_id))
                .collect::<Vec<_>>()
                .join("\n"),
        ).build();

    let button: Component = ComponentBuilder::button(ButtonObject {
        style: ButtonStyle::Danger as _,
        label: Some("Kick all the people mentioned above".to_string()),
        custom_id: Some("kick_all".to_string()),
        ..Default::default()
    })
    .unwrap();

    let mut msg = CreateMessageData::default();
    msg.embeds.push(embed);
    msg.content = "Do you really want to kick all these people?".to_string();

    int.reply(msg.add_components(vec![vec![button]]), true)
        .await;
}

#[descord::slash(description = "View your (or someone else's) rank in this server.")]
pub async fn rank(int: Interaction, #[doc = "User to fetch avatar from"] user: Option<User>) {
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
pub async fn leaderboard(int: Interaction) {
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

#[descord::slash(
    description = "Set a user's level, sets xp to zero",
    permissions = "administrator"
)]
pub async fn set_level(
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
                username: user.username.clone(),
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
