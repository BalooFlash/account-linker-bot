use std::error::Error;

use chrono::prelude::*;
use reqwest::Client;
use config::Config;

use modules::*;

pub const COMMANDS: [&'static str;4] = ["help", "link", "unlink", "unlinkall"];

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

impl Adapter {
    pub fn from_str(s: &str) -> Option<Adapter> {
        match s {
            "LinuxOrgRu" => Some(Adapter::LinuxOrgRu),
            _ => None,
        }
    }

    pub fn to_str(&self) -> String {
        match *self {
            Adapter::LinuxOrgRu => "LinuxOrgRu".to_owned(),
        }
    }
}

pub enum MarkdownType {
    GitHub,
    Matrix,
    Telegram,
}

// command syntax is: 
// /link LinuxOrgRu username
// /unlink LinuxOrgRu username
// /unlinkall username
#[derive(Debug)]
pub enum UpstreamUpdate {
    Link(UserInfo),
    Unlink { user_name: String, linked_user_name: String },
    UnlinkAll { user_name: String },
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
    fn as_markdown(&self, md_type: MarkdownType) -> String;
    fn as_html(&self) -> String;
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

    pub fn push(&self, client: &Client, user_info: &UserInfo, update: Box<UpdateDesc>) {
        match *self {
            Connector::Matrix {ref access_token, ..} => {
                matrix_org::post_message(client, access_token, &user_info.chat_id, update);
            }
        }
    }
}

impl Adapter {
    pub fn poll(&self,
                client: &Client,
                specifiers: Vec<String>)
                -> Result<Vec<Box<UpdateDesc>>, CoreError> {
        match *self {
            Adapter::LinuxOrgRu => {
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
        let linked_user_name = self.linked_user_name.to_owned();
        let update_result = self.adapter.poll(client, vec![linked_user_name]);
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

        // if we have last_update set to zero then this we are newly created user_info
        // in this case, fetch all updates from the adapter and don't report them,
        // instead, skip all the updates and set our timestamp to newest
        if self.last_update.timestamp() == 0 {
            info!("Updating newly created user info timestamp to latest available: {}", current_latest_update);
            self.last_update = current_latest_update;
            return Vec::default();
        }

        let new_updates: Vec<Box<UpdateDesc>> = updates.into_iter().filter(|u| u.timestamp() > self.last_update).collect();
        self.last_update = current_latest_update;

        new_updates
    }
}