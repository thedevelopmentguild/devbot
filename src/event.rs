use super::*;

#[descord::event]
pub async fn ready(data: ReadyData) {
    println!(
        "Logged in as: {}#{}",
        data.user.username, data.user.discriminator
    );
}

#[descord::event]
pub async fn guild_create(guild: GuildCreate) {
    let users = guild.members;
    let mut binding = DB.lock().await;
    let db = binding.as_mut().unwrap();

    for user in users {
        let user_id = &user.user.as_ref().unwrap().id;
        let user = user.user.as_ref().unwrap();
        let exists: bool = db.hexists(&guild.id, &user_id).unwrap();

        if !exists {
            let _: () = db
                .hset(
                    &guild.id,
                    user_id,
                    Data {
                        username: user.username.clone(),
                        user_id: user_id.to_string(),
                        level: 0,
                        xp: 0,
                        time: 0,
                    }
                    .serialize_json(),
                )
                .unwrap();
        }
    }
}

#[descord::event]
pub async fn member_join(member: Member) {
    println!("{:?}", member);
    let user = member.user.as_ref().unwrap();
    let user_id = user.id.clone();
    let username = user.username.clone();

    let _: () = db!()
        .hset(
            member.guild_id.as_ref().unwrap(),
            user_id.clone(),
            Data {
                username,
                user_id,
                level: 0,
                xp: 0,
                time: 0,
            }
            .serialize_json(),
        )
        .unwrap();
}

#[descord::event]
pub async fn member_leave(member: MemberLeave) {
    let id = member.user.id;
    let guild_id = member.guild_id;
    let _: () = db!().hdel(guild_id, id).unwrap();
}

#[descord::event]
pub async fn message_create(msg: Message) {
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

                vanish(
                    msg.reply(format!(
                        "> You just reached level **{}**!\n > XP: 0/{}",
                        userdata.level,
                        next_level_xp(userdata.level),
                    ))
                    .await,
                )
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
