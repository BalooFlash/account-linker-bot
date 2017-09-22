use reqwest::Client;

use config::Config;

use serde_json;

use itertools::Itertools;
use regex::Regex;
use chrono::prelude::*;
use uuid::Uuid;

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
#[serde(tag = "type", content = "content")]
enum EventContent {
    /// This event is used when sending messages in a room. Messages are not limited to be text.
    /// The msgtype key outlines the type of message, e.g. text, audio, image, video, etc.
    /// The body key is text and MUST be used with every kind of msgtype as a fallback mechanism for when
    /// a client cannot render a message. This allows clients to display something even if it is just plain text.
    /// For more information on msgtypes, see m.room.message msgtypes.
    #[serde(rename = "m.room.message")]
    Message(MessageEventContent),

    /// A room has an opaque room ID which is not human-friendly to read. A room alias is human-friendly,
    /// but not all rooms have room aliases. The room name is a human-friendly string designed to be displayed
    /// to the end-user. The room name is not unique, as multiple rooms can have the same room name set.
    #[serde(rename = "m.room.name")]
    Name { name: String },

    /// A topic is a short message detailing what is currently being discussed in the room.
    /// It can also be used as a way to display extra information about the room, which may not be
    /// suitable for the room name. The room topic can also be set when creating a room using /createRoom
    /// with the topic key.
    #[serde(rename = "m.room.topic")]
    Topic { topic: String },

    /// A picture that is associated with the room. This can be displayed alongside the room information.
    #[serde(rename = "m.room.avatar")]
    Avatar { url: String }, // not all fields are taken
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "msgtype")]
enum MessageEventContent {
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
    let login = conf.get_str("matrix.login").expect(
        "matrix.login property must be supplied in config",
    );
    let password = conf.get_str("matrix.password").expect(
        "matrix.password property must be supplied in config",
    );
    let post_body = Login {
        login_type: "m.login.password".to_owned(),
        user: login,
        password: password,
    };

    let login_url = MATRIX_API_ENDPOINT.to_owned() + "/login";
    let body_json = serde_json::to_string(&post_body)?;
    let mut response = client.post(&login_url)?.body(body_json).send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!(
            "Connect returned invalid code: {}",
            response.status()
        )));
    }

    let mut response_json = String::new();
    response.read_to_string(&mut response_json)?;
    let response_body: LoginAnswer = serde_json::from_str(&response_json)?;
    Ok(response_body.access_token)
}

pub fn process_updates(
    client: &Client,
    token: &String,
    last_batch: &mut String,
) -> Result<Vec<UpstreamUpdate>, CoreError> {
    // sync is the main routine in matrix.org lifecycle
    let sync_url = MATRIX_API_ENDPOINT.to_owned() + "/sync";
    let mut request_url = sync_url + "?access_token=" + token;
    if !last_batch.is_empty() {
        request_url = request_url + "&sync=" + last_batch;
    }

    let mut response = client.get(&request_url)?.send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!(
            "Connect returned invalid code: {}",
            response.status()
        )));
    }

    // receive sync object - events, invites etc
    let mut response_content = String::new();
    response.read_to_string(&mut response_content)?;
    let response_body: SyncAnswer = serde_json::from_str(&response_content)?;
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
        let mut all_updates: Vec<UpstreamUpdate> = vec![];
        for room_events in response_body.rooms.join {
            let all_commands = COMMANDS.iter().join("|");
            let command_regex = Regex::new(&format!(r"({})\s+(\w+)\s+(\w+)", all_commands))
                .expect("Must be valid regexp!");
            let room_id = room_events.0;
            let room_status = room_events.1.timeline;
            let updates_of_this_room: Vec<UpstreamUpdate> = room_status.events
                .iter() // command syntax is: /link LinuxOrgRu username
                .filter_map(|event| { // check that message text corresponds to regex
                    if let EventContent::Message(ref msg) = event.content {
                        if let &MessageEventContent::Text { ref body } = msg {
                            return command_regex
                                .captures(body) // ... and return it with the sender name
                                .map(|capture| (capture, &event.sender));
                        }
                    }
                    None
                })
                .filter_map(|capture| {
                    let user_name = capture.1;
                    let groups = capture.0;
                    let adapter = match &groups[1] {
                        "LinuxOrgRu" => Adapter::LinuxOrgRu,
                        _ => return None, // skip unknown adapters
                    };

                    Some(UpstreamUpdate::Link(UserInfo {  
                        user_id: 0, 
                        chat_id: room_id.to_owned(), 
                        user_name: user_name.to_owned(), 
                        linked_user_name: groups[2].to_owned(),
                        connector_type: "Matrix".to_owned(),
                        adapter: adapter,
                        last_update: FixedOffset::east(0).timestamp(0, 0),
                    }))
                })
                .collect();

            all_updates.extend(updates_of_this_room);
        }

        return Ok(all_updates);
    }

    Ok(Vec::default())
}

pub fn post_message(
    client: &Client,
    access_token: &String,
    chat_id: &String,
    text: String,
) -> Result<String, CoreError> {
    let uuid = Uuid::new_v4().hyphenated().to_string();
    let post_msg_url = MATRIX_API_ENDPOINT.to_owned() + "/rooms/" + chat_id + "/send/m.room.message/" + &uuid +
        "?access_token=" + access_token;

    let post_content = MessageEventContent::Notice { body: text };
    let body_json = serde_json::to_string(&post_content)?;

    let mut response = client.post(&post_msg_url)?.body(body_json).send()?;
    let mut response_content = String::new();
    response.read_to_string(&mut response_content)?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!(
            "Connect returned invalid code: {}",
            response.status()
        )));
    }

    let mut response_body: HashMap<String, String> = serde_json::from_str(&response_content)?;
    let event_id = response_body.remove("event_id").expect(
        "Answer must contain event id in case of successful response",
    );
    Ok(event_id)
}