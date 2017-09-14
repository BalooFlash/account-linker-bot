use reqwest::Client;
use reqwest::Response;

use config::Config;

use std::io::Read;
use std::result::Result;
use std::collections::HashMap;

use entities::CoreError;

use serde_json;

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
    invite: HashMap<String, RoomInviteState>,
}

#[derive(Serialize, Deserialize)]
struct RoomInviteState {
    // we don't need this for a bot
    //invite_state: RoomInviteEvents,
}

pub fn connect(client: &Client, conf: &Config) -> Result<String, CoreError> {
    let login = conf.get_str("matrix.login").expect("matrix.login property must be supplied in config");
    let password = conf.get_str("matrix.password").expect("matrix.password property must be supplied in config");
    let post_body = Login {
        login_type: "m.login.password".to_owned(),
        user: login,
        password: password
    };

    let login_url = MATRIX_API_ENDPOINT.to_owned() + "/login";
    let body_json = serde_json::to_string(&post_body)?;
    let mut response = client.post(&login_url)?.body(body_json).send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())))
    }

    let mut response_json = String::new();
    response.read_to_string(&mut response_json)?;
    let response_body: LoginAnswer = serde_json::from_str(&response_json)?;
    Ok(response_body.access_token)
}

pub fn process_updates(client: &Client, token: &String, last_batch: &mut String) -> Result<(), CoreError> {
    // sync is the main routine in matrix.org lifecycle
    let sync_url = MATRIX_API_ENDPOINT.to_owned() + "/sync";
    let mut request_url = sync_url + "?access_token=" + token;
    if !last_batch.is_empty() {
        request_url = request_url + "&sync=" + last_batch;
    }

    let mut response = client.get(&request_url)?.send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Connect returned invalid code: {}", response.status())))
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

    Ok(())
}