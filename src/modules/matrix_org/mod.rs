use reqwest::Client;

use config::Config;

use serde_json;
use chrono::prelude::*;
use uuid::Uuid;

use std::collections::HashMap;

mod matrix_api;

use entities::*;
use self::matrix_api::*;

const MATRIX_API_ENDPOINT: &str = "https://matrix.org/_matrix/client/r0";

pub fn connect(client: &Client, conf: &Config) -> Result<String> {
    let user = conf.get_str("matrix.login").expect("matrix.login property must be supplied in config");
    let password = conf.get_str("matrix.password").expect("matrix.password property must be supplied in config");
    let post_body = Login {
        login_type: "m.login.password".to_owned(),
        user,
        password,
    };

    let login_url = MATRIX_API_ENDPOINT.to_owned() + "/login";
    let body_json = serde_json::to_string(&post_body)?;
    let response = client.post(&login_url)?.body(body_json).send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
    }

    let response_body: LoginAnswer = serde_json::from_reader(response)?;
    Ok(response_body.access_token)
}

/// Get all updates since last batch from Matrix servers. This requires auth.
///
/// - Also join any room if invited
pub fn process_updates(client: &Client, token: &String, last_batch: &mut String) -> Result<Vec<UpstreamUpdate>> {
    // sync is the main routine in matrix.org lifecycle
    let sync_url = MATRIX_API_ENDPOINT.to_owned() + "/sync";
    let mut request_url = sync_url + "?access_token=" + token;
    if !last_batch.is_empty() {
        request_url = request_url + "&since=" + last_batch;
    }

    let response = client.get(&request_url)?.send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
    }

    // receive sync object - events, invites etc
    let response_body: SyncAnswer = serde_json::from_reader(response)?;
    *last_batch = response_body.next_batch;

    // process invites
    if !response_body.rooms.invite.is_empty() {
        for room_id in response_body.rooms.invite.keys() {
            let join_url = format!("{base}/join/{room_id}?access_token={token}", base = MATRIX_API_ENDPOINT, room_id = room_id,token = token);
            client.post(&join_url)?.send()?;
        }
    }

    // process link/unlink requests
    if !response_body.rooms.join.is_empty() {
        return capture_commands(response_body.rooms.join);
    }

    Ok(Vec::default())
}

/// Retrieves and parses commands from room updates.
///
/// Skips any updates that are not `m.message` type.
fn capture_commands(all_rooms: HashMap<String, RoomJoinState>) -> Result<Vec<UpstreamUpdate>> {
    let mut all_updates: Vec<UpstreamUpdate> = vec![];
    for room_events in all_rooms {
        let room_id = room_events.0;
        let room_status = room_events.1.timeline;
        for event in room_status.events {
            let body = match event.content {
                EventContent::Message(MessageEventContent::Text { ref body }) => body.to_owned(),
                _ => continue,
            };

            // we start Matrix commands with exclamation mark
            // because slash is reserved with server communication
            if !body.starts_with("!") {
                continue;
            }

            let arguments: Vec<&str> = body.trim_left_matches("!").split(" ").collect();
            match parse_command(&room_id, event, arguments) {
                Some(update) => all_updates.push(update),
                None => warn!("Couldn't parse command: {}", body),
            }
        }
    }

    return Ok(all_updates);
}

/// parse command and build upstream update entity from it if command is valid
fn parse_command(room_id: &str, event: Event, mut arguments: Vec<&str>) -> Option<UpstreamUpdate> {
    if arguments.is_empty() {
        return None;
    }

    // helper lambda to build UserInfo from event + command
    let info_from_event = |event: Event, args: &Vec<&str>| {
        if args.len() < 2 {
            return None;
        }

        let adapter: Result<Adapter> = str::parse(&args[0]);
        let linked_user_id = args[1];
        if adapter.is_err() {
            return None;
        }

        Some(UserInfo {
            id: 0,
            upstream_type: "Matrix".to_owned(),
            chat_id: room_id.to_owned(),
            user_id: event.sender.to_owned(),
            adapter: adapter.unwrap(),
            linked_user_id: linked_user_id.to_owned(),
            last_update: NaiveDateTime::from_timestamp(0, 0),
            verified: false
        })
    };

    match arguments.remove(0) {
        "link" => info_from_event(event, &arguments).map(|info| UpstreamUpdate::Link(info)),
        "unlink" => info_from_event(event, &arguments).map(|info| UpstreamUpdate::Unlink(info)),
        "unlinkall" => {
            Some(UpstreamUpdate::UnlinkAll {
                upstream_type: "Matrix".to_owned(),
                user_name: event.sender,
            })
        }

        _ => None,
    }
}

/// Posts update as formatted `m.notice` text message. This requires auth.
///
/// This uses undocumented `org.matrix.custom.html` format,
/// so is subject to change in future once markdown/other formatting solution is in place.
pub fn post_update(client: &Client, access_token: &str, chat_id: &str, update: Box<UpdateDesc>) -> Result<String> {
    let uuid = Uuid::new_v4().hyphenated().to_string();
    let post_msg_url = MATRIX_API_ENDPOINT.to_owned() + "/rooms/" + chat_id + "/send/m.room.message/" + &uuid +
                       "?access_token=" + access_token;

    let post_content = MessageEventContent::Notice {
        body: update.as_string(),
        format: Some("org.matrix.custom.html".to_owned()),
        formatted_body: Some(update.as_html()),
    };
    let body_json = serde_json::to_string(&post_content)?;

    let response = client.put(&post_msg_url)?.body(body_json).send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
    }

    let mut response_body: HashMap<String, String> = serde_json::from_reader(response)?;
    let event_id = response_body.remove("event_id")
        .expect("Answer must contain event id in case of success");
    Ok(event_id)
}

/// Get user display name given we know their user name slug.
/// Auth is not required for this.
pub fn get_display_name(client: &Client, user_name: &str) -> Result<String> {
    let get_url = MATRIX_API_ENDPOINT.to_owned() + "/profile/" + user_name + "/displayname";

    let response = client.get(&get_url)?.send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Matrix returned invalid code: {}", response.status())));
    }

    // receive sync object - events, invites etc
    let mut response_body: HashMap<String, String> = serde_json::from_reader(response)?;
    let display_name = response_body.remove("displayname").expect("Answer mustcontain displayname in case of success");
    Ok(display_name)
}

/// Posts a plain `m.notice` message with requested text. Requires auth.
pub fn post_plain_message(client: &Client, access_token: &str, chat_id: &String, message: String) -> Result<String> {
    let uuid = Uuid::new_v4().hyphenated().to_string();
    let post_msg_url = MATRIX_API_ENDPOINT.to_owned() + "/rooms/" + chat_id + "/send/m.room.message/" + &uuid +
                       "?access_token=" + access_token;
    let post_content = MessageEventContent::Notice {
        body: message,
        format: None,
        formatted_body: None,
    };
    let body_json = serde_json::to_string(&post_content)?;

    let response = client.put(&post_msg_url)?.body(body_json).send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
    }

    let mut response_body: HashMap<String, String> = serde_json::from_reader(response)?;
    let event_id = response_body.remove("event_id")
        .expect("Answer must contain event id in case of success");
    Ok(event_id)
}