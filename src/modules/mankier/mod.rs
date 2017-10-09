use std::io::Read;

use reqwest::Client;

use entities::Result;
use entities::CoreError;

const MANKIER_ENDPOINT: &str = "https://www.mankier.com/api/v2/explain/?q=";

pub fn explain_command(client: &Client, command: &str) -> Result<String> {
    let req_url = MANKIER_ENDPOINT.to_owned() + command;

    let mut response = client.get(&req_url)?.send()?;
    if !response.status().is_success() {
        return Err(CoreError::CustomError(format!("Explain returned invalid code: {}", response.status())));
    }

    let mut explanation = String::new();
    response.read_to_string(&mut explanation);
    Ok(explanation)
}