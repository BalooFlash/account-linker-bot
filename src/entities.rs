use chrono::prelude::*;
use modules::*;
use reqwest::Client;

// Where do we request updates to be sent to
// and from where do we connect to link accounts
pub enum Connector {
    Matrix,
}

// Where do we retrieve updates from
pub enum Adapter {
    #[cfg(feature = "linux-org-ru")]
    LinuxOrgRu,
}

// Common trait that both Connectors and Adapters possess
trait Connectable {
    fn connect(&self);
}

trait Pollable: Connectable {
    fn poll<T: ToString>(&self, client: &Client, specifiers: &Vec<String>) -> Vec<T>;
}

trait Notifiable: Connectable {
    fn send(&self, comment: UserComment);
}

impl Connectable for Connector {
    fn connect(&self) {
        match self {
            Matrix => {}
        }
    }
}

impl Notifiable for Connector {
    fn send(&self, comment: UserComment) {
        match self {
            Matrix => {}
        }
    }
}

impl Connectable for Adapter {
    fn connect(&self) {
        match self {
            LinuxOrgRu => {
                // nothing is needed
            }
        }
    }
}

impl Pollable for Adapter {
    fn poll<T: ToString>(&self, client: &Client, specifiers: &Vec<String>) -> Vec<T> {
        match self {
            LinuxOrgRu => {
                let user_name = specifiers.into_iter().next();
                return lor_ru::get_user_posts(&user_name, client);
            }
        }
    }
}

// User info struct, which provides a link between Connector and Adapter
// UserInfo struct instances are meant to be alive almost the same amount of time
// the application is running.
#[derive(new)]
pub struct UserInfo {
    user_id: i64, // internal user ID as saved in DB, mostly not used
    user_name: String, // user name as provided by Connector
    linked_user_name: String, // linked user name, as requested from Adapter
    connector: Connector, // Connector itself, most of the time it's in `connected` state
    adapter: Adapter, // Adapter itself, most of the time it's in `connected` state
    last_update: DateTime<Local>, // Last time update was queried for this instance
}