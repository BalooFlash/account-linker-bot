use std::error::Error;

use chrono::prelude::*;
use reqwest::Client;
use config::Config;

use modules::*;

pub const COMMANDS: [&'static str;3] = ["/link", "/unlink", "/clear_links"];

#[derive(Debug, Error)]
pub enum CoreError {
    // Error retrieving LOR HTML page
    HttpError(::reqwest::Error),
    // Error converting response to string
    ConvertError(::std::io::Error),
    // Error (de) serializing data
    JsonSerializeError(::serde_json::Error),
    // Our own error
    #[error(msg_embedded, non_std, no_from)]
    CustomError(String),
}


// Where do we request updates to be sent to
// and from where do we connect to link accounts
pub enum Connector {
    Matrix {
        access_token: String,
        last_batch: String,
    },
}

// Where do we retrieve updates from
#[derive(Debug)]
pub enum Adapter {
    #[cfg(feature = "linux-org-ru")]
    LinuxOrgRu,
}

pub enum MarkdownType {
    GitHub,
    Matrix,
    Telegram,
}

#[derive(Debug)]
pub enum UpstreamUpdate {
    Link(UserInfo),
    Unlink { user_name: String, linked_user_name: String, adapter: Adapter },
}

// User info struct, which provides a link between Connector and Adapter
// UserInfo struct instances are meant to be alive almost the same amount of time
// the application is running.
#[derive(Debug, new)]
pub struct UserInfo {
    pub user_id: i64, // internal user ID as saved in DB, mostly not used
    pub chat_id: String, // chat in which to post updates
    pub user_name: String, // user name as provided by Connector
    pub linked_user_name: String, // linked user name, as requested from Adapter
    pub connector_type: String, // connector descriptor
    pub adapter: Adapter, // Adapter itself, most of the time it's in `connected` state
    pub last_update: DateTime<FixedOffset>, // Last time update was queried for this instance
}


/// Update description
pub trait UpdateDesc {
    fn as_string(&self) -> String;
    fn as_markdown(&self, md_type: &MarkdownType) -> String;
    fn timestamp(&self) -> DateTime<FixedOffset>;
}

impl Connector {
    pub fn connect(&mut self, client: &Client, cfg: &Config) {
        match *self {
            Connector::Matrix { access_token: ref mut token, last_batch: _ } => {
                if token.is_empty() {
                    *token = matrix_org::connect(client, cfg).unwrap_or_default()
                }
            }
        };
    }

    pub fn check_updates(&mut self, client: &Client) -> Result<Vec<UpstreamUpdate>, CoreError> {
        match *self {
            Connector::Matrix { ref access_token, ref mut last_batch } => {
                matrix_org::process_updates(client, access_token, last_batch)
            }
        }
    }

    pub fn push(&self, client: &Client, updates: Vec<Box<UpdateDesc>>) {
        match self {
            Matrix => {}
        }
    }
}

impl Adapter {
    pub fn poll(&self,
                client: &Client,
                specifiers: Vec<String>)
                -> Result<Vec<Box<UpdateDesc>>, CoreError> {
        match self {
            LinuxOrgRu => {
                let user_name = specifiers.into_iter().next().unwrap();
                return lor_ru::get_user_posts(&user_name, client).map(|comments| {
                    comments.into_iter()
                        .map(|c| Box::new(c) as Box<UpdateDesc>)
                        .collect()
                });
            }
        }
    }
}

impl UserInfo {
    pub fn poll(&mut self, client: &Client) -> Vec<Box<UpdateDesc>> {
        let user_name = self.user_name.to_owned();
        let update_result = self.adapter.poll(client, vec![user_name]);
        let updates = match update_result {
            Err(error) => {
                error!("Error while polling: {}", error.description());
                return Vec::default()
            }
            Ok(updates) => {
                if updates.is_empty() {
                    info!("Nothing found for {}...", self.linked_user_name);
                    return Vec::default()
                }
                updates
            }
        };
        let current_latest_update = updates.iter().map(|u| u.timestamp()).max().unwrap();
        if self.last_update == current_latest_update {
            info!("No updates for {}...", self.linked_user_name);
            return Vec::default()
        }

        let new_updates: Vec<Box<UpdateDesc>> = updates.into_iter().filter(|u| u.timestamp() > self.last_update).collect();
        self.last_update = current_latest_update;

        new_updates
    }
}