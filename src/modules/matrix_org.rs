use reqwest::Client;
use reqwest::Response;

use config::Config;

use serde_json;

use itertools::Itertools;
use regex::Regex;
use chrono::prelude::*;

use std::io::Read;
use std::result::Result;
use std::collections::HashMap;

use entities::*;

const MATRIX_API_ENDPOINT: &'static str = "https://matrix.org/_matrix/client/r0";

#[derive(Serialize, Deserialize)]
struct Login {
    #[serde(rename = "type")]
    login_type: String,
    user: String,
    password: String,
}

#[derive(Serialize, Deserialize)]
struct LoginAnswer {
    access_token: String,
    home_server: String,
    user_id: String,
    device_id: String,
}

#[derive(Serialize, Deserialize)]
struct SyncAnswer {
    rooms: RoomUpdates,
    next_batch: String,
}

#[derive(Serialize, Deserialize)]
struct RoomUpdates {
    /// The rooms that the user has been invited to.
    invite: HashMap<String, RoomInviteState>,

    /// The rooms that the user has joined.
    join: HashMap<String, RoomJoinState>,
}

#[derive(Serialize, Deserialize)]
struct RoomInviteState {
    // we don't need this for a bot
    //invite_state: RoomInviteEvents,
}

#[derive(Serialize, Deserialize)]
struct RoomJoinState {
    /// The timeline of messages and state changes in the room.
    timeline: Timeline, 

    // we don't need those yet
    //state: EventsBatch,
    //ephemeral: EventsBatch,
    //account_data: EventsBatch,
}

/// The timeline of messages and state changes in the room.
#[derive(Serialize, Deserialize)]
struct Timeline {
    /// True if the number of events returned was limited by the limit on the filter
    limited: bool,

    /// A token that can be supplied to to the from parameter of the rooms/{roomId}/messages endpoint
    prev_batch: String,

    /// List of events
    events: Vec<Event>,
}


#[derive(Serialize, Deserialize)]
struct Event {
    ///  The globally unique event identifier.
    event_id: String,

    /// The type of event.
    #[serde(rename = "type")]
    event_type: String,

    /// Timestamp in milliseconds on originating homeserver when this event was sent.
    origin_server_ts: u64,

    /// The MXID of the user who sent this event.
    sender: String,

    /// Information about this event which was not sent by the originating homeserver
    unsigned: Unsigned,

    /// The content of this event. The fields in this object will vary depending on the type of event.
    content: EventContent,

    /// Optional. This key will only be present for state events.
    /// A unique key which defines the overwriting semantics for this piece of room state.
    state_key: Option<String>,
}

/// Information about event which was not sent by the originating homeserver
#[derive(Serialize, Deserialize)]
struct Unsigned {
    /// Optional. The previous content for this state.
    /// This will be present only for state events appearing in the timeline.
    /// If this is not a state event, or there is no previous content, this key will be missing.
    prev_content: Option<EventContent>,

    /// Time in milliseconds since the event was sent.
    age: u32,

    /// Optional. The transaction ID set when this message was sent.
    /// This key will only be present for message events sent by the device calling this API.
    transaction_id: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "msgtype")]
enum EventContent {
    /// message is the most basic message and is used to represent text.
    #[serde(rename = "m.text")]
    Text { body: String },

    /// message is similar to m.text except that the sender is 'performing' the action contained in the body key,
    /// similar to /me in IRC. This message should be prefixed by the name of the sender. This message could also
    ///  be represented in a different colour to distinguish it from regular m.text messages.
    #[serde(rename = "m.emote")]
    Emote { body: String },

    /// A m.notice message should be considered similar to a plain m.text message except that clients should visually
    /// distinguish it in some way. It is intended to be used by automated clients, such as bots, bridges, and other entities,
    /// rather than humans. Additionally, such automated agents which watch a room for messages and respond to them ought to ignore
    /// m.notice messages. This helps to prevent infinite-loop situations where two automated clients continuously exchange
    /// messages, as each responds to the other.
    #[serde(rename = "m.notice")]
    Notice { body: String },

    /// This message represents a single image and an optional thumbnail.
    #[serde(rename = "m.image")]
    Image {
        body: String,
        url: String,
        thumbnail_url: Option<String>,
        info: Option<ImageInfo>,
        thumbnail_info: Option<ImageInfo>,
    },

    /// This message represents a generic file.
    #[serde(rename = "m.file")]
    File {
        body: String,
        filename: String,
        info: Option<FileInfo>,
        thumbnail_info: Option<ImageInfo>,
        thumbnail_url: Option<String>,
        url: String,
    }, 

    // m.location, m.video, m.audio are not so interesting for us
}

/// Metadata about the image referred to
#[derive(Serialize, Deserialize)]
struct ImageInfo {
    /// The mimetype of the image, e.g. image/jpeg.
    mimetype: String,

    /// The height of the image in pixels.
    h: u32,

    /// The width of the image in pixels.
    w: u32,

    /// Size of the image in bytes.
    size: u64, 

    // orientation of image, undocumented
    //orientation: u32,
}

/// Information about the file referred to
#[derive(Serialize, Deserialize)]
struct FileInfo {
    /// The mimetype of the file, e.g. image/jpeg.
    mimetype: String,

    /// The size of the file in bytes.
    size: u64,
}

pub fn connect(client: &Client, conf: &Config) -> Result<String, CoreError> {
    let login = conf.get_str("matrix.login")
        .expect("matrix.login property must be supplied in config");
    let password = conf.get_str("matrix.password")
        .expect("matrix.password property must be supplied in config");
    let post_body = Login {
        login_type: "m.login.password".to_owned(),
        user: login,
        password: password,
    };

    let login_url = MATRIX_API_ENDPOINT.to_owned() + "/login";
    let body_json = serde_json::to_string(&post_body)?;
    let mut response = client.post(&login_url)?.body(body_json).send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
    }

    let mut response_json = String::new();
    response.read_to_string(&mut response_json)?;
    let response_body: LoginAnswer = serde_json::from_str(&response_json)?;
    Ok(response_body.access_token)
}

pub fn process_updates(client: &Client,
                       token: &String,
                       last_batch: &mut String)
                       -> Result<Vec<UpstreamUpdate>, CoreError> {
    // sync is the main routine in matrix.org lifecycle
    let sync_url = MATRIX_API_ENDPOINT.to_owned() + "/sync";
    let mut request_url = sync_url + "?access_token=" + token;
    if !last_batch.is_empty() {
        request_url = request_url + "&sync=" + last_batch;
    }

    let mut response = client.get(&request_url)?.send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
    }

    // receive sync object - events, invites etc
    let mut response_json = String::new();
    response.read_to_string(&mut response_json)?;
    let response_body: SyncAnswer = serde_json::from_str(&response_json)?;
    *last_batch = response_body.next_batch;

    // process invites
    if !response_body.rooms.invite.is_empty() {
        for room_id in response_body.rooms.invite.keys() {
            let join_url = MATRIX_API_ENDPOINT.to_owned() + "/join/" + room_id + "?access_token=" + token;
            client.post(&join_url)?.send()?;
        }
    }

    // process link/unlink requests
    if !response_body.rooms.join.is_empty() {
        for room_events in response_body.rooms.join {
            let all_commands = COMMANDS.iter().join("|");
            let command_regex = Regex::new(&format!(r"({})\s+(\w+)\s+(\w+)", all_commands))
                .expect("Must be valid regexp!");
            let room_id = room_events.0;
            let room_status = room_events.1.timeline;
            room_status.events
                .iter() // command syntax is: /link LinuxOrgRu username
                .filter_map(|event| match event.content { 
                    EventContent::Text { ref body } => 
                        command_regex
                            .captures(body)
                            .map(|capture| (capture, event.sender)), 
                    _ => None 
                    })
                .map(|capture| {
                    let user_name = capture.1;
                    let groups = capture.0;
                    let adapter = match &groups[1] {
                        "LinuxOrgRu" => Some(Adapter::LinuxOrgRu),
                        _ => None,
                    };

                    UserInfo {  user_id: 0, 
                                chat_id: room_id, 
                                user_name: user_name, 
                                linked_user_name: groups[2].to_owned(),
                                connector_type: "Matrix".to_owned(),
                                adapter: adapter.unwrap(),
                                last_update: Utc.timestamp(0, 0),
                    }
                });
        }
    }

    Ok(Vec::default())
}