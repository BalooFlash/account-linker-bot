use chrono::prelude::*;
use reqwest::Client;
use config::Config;

use modules::*;

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
    Matrix { access_token: String },
}

// Where do we retrieve updates from
pub enum Adapter {
    #[cfg(feature = "linux-org-ru")]
    LinuxOrgRu,
}

pub enum MarkdownType {
    GitHub,
    Matrix,
    Telegram
}

/// Common trait that both Connectors and Adapters possess
pub trait Connectable {
    fn connect(&self, client: &Client, cfg: &Config);
}

/// Update description
pub trait UpdateDesc {
    fn as_string(&self) -> String;
    fn as_markdown(&self, md_type: &MarkdownType) -> String;
}

impl Connectable for Connector {
    fn connect(&self, client: &Client, cfg: &Config) {
        match self {
            Matrix => matrix_org::connect(client, cfg),
        };
    }
}

impl Connectable for Adapter {
    fn connect(&self, client: &Client, cfg: &Config) {
        match self {
            LinuxOrgRu => {
                // nothing is needed
            }
        }
    }
}

impl Adapter {
    pub fn poll(&self, client: &Client, specifiers: &Vec<String>) -> Result<Vec<Box<UpdateDesc>>, CoreError> {
        match self {
            LinuxOrgRu => {
                let user_name = specifiers.into_iter().next().unwrap();
                return lor_ru::get_user_posts(&user_name, client)
                    .map(|comments| comments.into_iter()
                        .map(|c| Box::new(c) as Box<UpdateDesc>)
                        .collect());
            }
        }
    }
}

impl Connector {
    pub fn push(&self, client: &Client, updates: Vec<Box<UpdateDesc>>) {
        match self {
            Matrix => {
                
            }
        }
    }
}

// User info struct, which provides a link between Connector and Adapter
// UserInfo struct instances are meant to be alive almost the same amount of time
// the application is running.
#[derive(new)]
pub struct UserInfo {
    pub user_id: i64, // internal user ID as saved in DB, mostly not used
    pub user_name: String, // user name as provided by Connector
    pub linked_user_name: String, // linked user name, as requested from Adapter
    pub connector: Connector, // Connector itself, most of the time it's in `connected` state
    pub adapter: Adapter, // Adapter itself, most of the time it's in `connected` state
    pub last_update: DateTime<Local>, // Last time update was queried for this instance
}