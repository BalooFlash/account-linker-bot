use reqwest::Client;

use config::Config;

use serde_json::{self, Value};

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

    /// The type of event. This SHOULD be namespaced
    #[serde(rename = "type")]
    event_type: String,

    /// Timestamp in milliseconds on originating homeserver when this event was sent.
    origin_server_ts: u64,

    /// The MXID of the user who sent this event.
    sender: String,

    /// Event that this event redacts
    redacts: Option<String>,

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
#[serde(untagged)]
enum EventContent {
    /// This event is sent by a homeserver directly to inform of changes to the list of aliases it knows about for that room.
    /// The state_key for this event is set to the homeserver which owns the room alias. The entire set of known aliases for the room
    /// is the union of all the m.room.aliases events, one for each homeserver. Clients should check the validity of any room alias given
    /// in this list before presenting it to the user as trusted fact. The lists given by this event should be considered simply as advice
    /// on which aliases might exist, for which the client can perform the lookup to confirm whether it receives the correct room ID.
    Aliases { aliases: Vec<String> },

    /// This event is used to inform the room about which alias should be considered the canonical one.
    /// This could be for display purposes or as suggestion to users which alias to use to advertise the room.
    CanonicalAlias { alias: String },

    /// This event is used to inform the room about which alias should be considered the canonical one.
    /// This could be for display purposes or as suggestion to users which alias to use to advertise the room.
    Tag { tags: HashMap<String, Tag> },

    /// Informs the client of a user's presence state change.
    Presense {
        user_id: String,
        presense: String,
        avatar_url: Option<String>,
        last_active_ago: Option<u64>,
        currently_active: Option<bool>,
        displayname: Option<String>,
    },

    /// This event controls whether a user can see the events that happened in a room from before they joined.
    HistoryVisibility { history_visibility: String },

    /// This is the first event in a room and cannot be changed. It acts as the root of all other events.
    Create {
        #[serde(rename = "m.federate")]
        federate: Option<bool>,
        creator: String,
    },

    /// A room may be public meaning anyone can join the room without any prior action.
    /// Alternatively, it can be invite meaning that a user who wishes to join the room must first receive an invite to the room from someone
    /// already inside of the room. Currently, knock and private are reserved keywords which are not implemented.
    JoinRules { join_rule: String }, //  ["public", "knock", "invite", "private"]

    /// Adjusts the membership state for a user in a room. It is preferable to use the membership APIs (/rooms/<room id>/invite etc)
    /// when performing membership actions rather than adjusting the state directly as there are a restricted set of valid transformations.
    /// For example, user A cannot force user B to join a room, and trying to force this state change directly will fail.
    Member {
        // third_party_invite: Invite
        membership: String, // ["invite", "join", "knock", "leave", "ban"]
        avatar_url: Option<String>,
        displayname: Option<String>,
    },

    /// This event specifies the minimum level a user must have in order to perform a certain action. It also specifies the levels of each user in the room.
    PowerLevels {
        events_default: u32,
        invite: u32,
        state_default: u32,
        redact: u32,
        ban: u32,
        users_default: u32,
        events: HashMap<String, u32>,
        kick: u32,
        users: HashMap<String, u32>,
    },

    /// Events can be redacted by either room or server admins. Redacting an event means that all keys not required by the protocol are stripped off,
    /// allowing admins to remove offensive or illegal content that may have been attached to any event. This cannot be undone, allowing server owners to
    /// physically delete the offending data. There is also a concept of a moderator hiding a message event, which can be undone, but cannot be applied to state events.
    /// The event that has been redacted is specified in the redacts event level key.
    Redaction { reason: String },

    /// This event is used when sending messages in a room. Messages are not limited to be text.
    /// The msgtype key outlines the type of message, e.g. text, audio, image, video, etc.
    /// The body key is text and MUST be used with every kind of msgtype as a fallback mechanism for when
    /// a client cannot render a message. This allows clients to display something even if it is just plain text.
    /// For more information on msgtypes, see m.room.message msgtypes.
    Message(MessageEventContent),

    /// Informs the client of the list of users currently typing.
    Typing { user_ids: Vec<String> },

    /// A room has an opaque room ID which is not human-friendly to read. A room alias is human-friendly,
    /// but not all rooms have room aliases. The room name is a human-friendly string designed to be displayed
    /// to the end-user. The room name is not unique, as multiple rooms can have the same room name set.
    Name { name: String },

    /// A topic is a short message detailing what is currently being discussed in the room.
    /// It can also be used as a way to display extra information about the room, which may not be
    /// suitable for the room name. The room topic can also be set when creating a room using /createRoom
    /// with the topic key.
    Topic { topic: String },

    /// A picture that is associated with the room. This can be displayed alongside the room information.
    Avatar { url: String }, // not all fields are taken

    Other(Value),
}

#[derive(Serialize, Deserialize)]
struct Tag {
    order: Option<u32>,
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
    Notice {
        body: String,
        format: Option<String>,
        formatted_body: Option<String>,
    },

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
        url: String,
        info: Option<FileInfo>,
        thumbnail_info: Option<ImageInfo>,
        thumbnail_url: Option<String>,
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
    let login = conf.get_str("matrix.login").expect("matrix.login property must be supplied in config");
    let password = conf.get_str("matrix.password").expect("matrix.password property must be supplied in config");
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
        request_url = request_url + "&since=" + last_batch;
    }

    let mut response = client.get(&request_url)?.send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
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
        return capture_commands(response_body.rooms.join);
    }

    Ok(Vec::default())
}

/// Retrieves and parses commands from room updates
fn capture_commands(all_rooms: HashMap<String, RoomJoinState>) -> Result<Vec<UpstreamUpdate>, CoreError> {
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

    let info_from_event = |event: Event, args: &Vec<&str>| {
        if args.len() < 2 {
            return None;
        }

        let adapter = Adapter::from_str(&args[1]);
        let linked_user_name = args[2];
        if adapter.is_none() {
            return None;
        }

        Some(UserInfo {
            user_id: 0,
            chat_id: room_id.to_owned(),
            user_name: event.sender.to_owned(),
            linked_user_name: linked_user_name.to_owned(),
            upstream_type: "Matrix".to_owned(),
            adapter: adapter.unwrap(),
            last_update: FixedOffset::east(0).timestamp(0, 0),
        })
    };

    match arguments.remove(0) {
        "link" => info_from_event(event, &arguments).map(|info| UpstreamUpdate::Link(info)),
        "unlink" => info_from_event(event, &arguments).map(|info| UpstreamUpdate::Unlink(info)),
        "unlinkall" => Some(UpstreamUpdate::UnlinkAll { upstream_type: "Matrix".to_owned(), user_name: event.sender }),
        _ => None,
    }
}

pub fn post_message(client: &Client,
                    access_token: &String,
                    chat_id: &String,
                    update: Box<UpdateDesc>)
                    -> Result<String, CoreError> {
    let uuid = Uuid::new_v4().hyphenated().to_string();
    let post_msg_url = MATRIX_API_ENDPOINT.to_owned() + "/rooms/" + chat_id + "/send/m.room.message/" + &uuid +
                       "?access_token=" + access_token;

    let post_content = MessageEventContent::Notice {
        body: update.as_string(),
        format: Some("org.matrix.custom.html".to_owned()),
        formatted_body: Some(update.as_html()),
    };
    let body_json = serde_json::to_string(&post_content)?;

    let mut response = client.put(&post_msg_url)?.body(body_json).send()?;
    let mut response_content = String::new();
    response.read_to_string(&mut response_content)?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())));
    }

    let mut response_body: HashMap<String, String> = serde_json::from_str(&response_content)?;
    let event_id = response_body.remove("event_id")
        .expect("Answer must contain event id in case of successful response");
    Ok(event_id)
}

#[cfg(test)]
mod tests {
    use serde_json;

    use std::path::Path;
    use std::fs::File;
    use std::io::Read;

    use modules::matrix_org::SyncAnswer;

    #[test]
    fn test_deserialize() {
        let mut file = File::open("tests/bot-response.json").expect("File must be present!");
        let mut get_body = String::new();
        file.read_to_string(&mut get_body).expect("File must be present!");
        let response_body: SyncAnswer = serde_json::from_str(&get_body).expect("File must be deserializable!");
    }

}