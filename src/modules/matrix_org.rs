use reqwest::Client;
use reqwest::Response;

use config::Config;

use std::io::Read;
use std::result::Result;
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

pub fn connect(client: &Client, conf: &Config) -> Result<(), CoreError> {
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
    Ok(())
}

pub fn post() -> Result<(), CoreError> {
    Ok(())
}