use super::*;

#[descord::event]
pub async fn ready(data: ReadyData) {
    println!(
        "Logged in as: {}#{}",
        data.user.username, data.user.discriminator
    );
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
