use std::error::Error;
use std::result;
use std::str;
use std::str::FromStr;

use chrono::prelude::*;
use reqwest::Client;
use config::Config;

use diesel::expression::AsExpression;
use diesel::expression::helper_types::AsExprOf;
use diesel::sqlite::Sqlite;
use diesel::types::Text;
use diesel::row::Row;
use diesel::types::FromSqlRow;
use database::schema::user_info;

use modules::*;

pub type Result<T> = result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    /// Error retrieving LOR HTML page
    HttpError(::reqwest::Error),
    /// Error converting response to string
    ConvertError(::std::io::Error),
    /// Error (de) serializing data
    JsonSerializeError(::serde_json::Error),
    /// Our own error
    #[error(msg_embedded, non_std, no_from)]
    CustomError(String),
}


// Where do we request updates to be sent to
// and from where do we connect to link accounts
pub enum Upstream {
    Matrix {
        access_token: String,
        last_batch: String,
    },
}

// Where do we retrieve updates from
#[derive(Debug, PartialEq, Hash, Eq)]
pub enum Adapter {
    #[cfg(feature = "linux-org-ru")]
    LinuxOrgRu,
}

impl FromStr for Adapter {
    type Err = CoreError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "LinuxOrgRu" => Ok(Adapter::LinuxOrgRu),
            _ => Err(CoreError::CustomError("No such adapter!".to_owned())),
        }
    }
}

impl ToString for Adapter {
    fn to_string(&self) -> String {
        match *self {
            Adapter::LinuxOrgRu => "LinuxOrgRu".to_owned(),
        }
    }
}

impl<'a> AsExpression<Text> for &'a Adapter {
    type Expression = AsExprOf<String, Text>;
    fn as_expression(self) -> Self::Expression {
        <String as AsExpression<Text>>::as_expression(self.to_string())
    }
}

impl FromSqlRow<Text, Sqlite> for Adapter {
    fn build_from_row<R: Row<Sqlite>>(row: &mut R) -> result::Result<Self, Box<Error + Send + Sync>> {
        let raw = <String as FromSqlRow<Text, Sqlite>>::build_from_row(row)?;
        let adapter: Result<Adapter> = str::parse(&raw);
        adapter.or_else(|e| Err(e.description().into()))
    }
}


impl Adapter {
    pub fn poll(&self, client: &Client, specifiers: Vec<String>) -> Result<Vec<Box<UpdateDesc>>> {
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

pub enum MarkdownType {
    GitHub,
    Matrix,
    Telegram,
}

/// command syntax is:
/// ```
/// /link LinuxOrgRu username
/// /unlink LinuxOrgRu username
/// /unlinkall username
/// ```
#[derive(Debug)]
pub enum UpstreamUpdate {
    Link(UserInfo),
    Unlink(UserInfo),
    UnlinkAll {
        /// From which upstream does this request come from
        upstream_type: String,
        /// Which user to process unlink for
        user_name: String,
    },
}

/// User info struct, which provides a link between Connector and Adapter
/// UserInfo struct instances are meant to be alive almost the same amount of time
/// the application is running.
#[derive(Debug, Hash, Eq, Queryable, Insertable)]
#[table_name = "user_info"]
pub struct UserInfo {
    /// internal id as saved in DB, mostly not used
    pub id: i32,
    /// upstream itself
    pub upstream_type: String,
    /// chat in which to post updates
    pub chat_id: String,
    /// user id/name as provided by Upstream
    pub user_id: String,
    /// Adapter itself, most of the time it's in `connected` state
    pub adapter: Adapter,
    /// linked user name, as requested from Adapter
    pub linked_user_id: String,
    /// Last time update was queried for this instance
    pub last_update: NaiveDateTime,
}

impl PartialEq for UserInfo {
    /// We don't compare internal ids
    fn eq(&self, rhs: &UserInfo) -> bool {
        self.chat_id == rhs.chat_id && self.user_id == rhs.user_id && self.linked_user_id == rhs.linked_user_id &&
        self.upstream_type == rhs.upstream_type && self.adapter == rhs.adapter
    }
}


/// Update description
pub trait UpdateDesc {
    fn as_string(&self) -> String;
    fn as_markdown(&self, md_type: MarkdownType) -> String;
    fn as_html(&self) -> String;
    fn timestamp(&self) -> NaiveDateTime;
}

impl Upstream {
    pub fn connect(&mut self, client: &Client, cfg: &Config) {
        match *self {
            Upstream::Matrix { access_token: ref mut token, last_batch: _ } => {
                if token.is_empty() {
                    *token = matrix_org::connect(client, cfg).unwrap_or_default()
                }
            }
        };
    }

    pub fn check_updates(&mut self, client: &Client) -> Result<Vec<UpstreamUpdate>> {
        match *self {
            Upstream::Matrix { ref access_token, ref mut last_batch } => {
                matrix_org::process_updates(client, access_token, last_batch)
            }
        }
    }

    pub fn push_update(&self, client: &Client, chat_id: &str, update: Box<UpdateDesc>) {
        match *self {
            Upstream::Matrix { ref access_token, .. } => {
                let result = matrix_org::post_update(client, access_token, chat_id, update);
                match result {
                    Ok(event_id) => info!("Message posted with event id {}", event_id),
                    Err(error) => error!("Error while sending Matrix message: {:?}", error),
                }
            }
        }
    }

    pub fn report_duplicate_link(&self, client: &Client, link: UserInfo) {
        match *self {
            Upstream::Matrix { ref access_token, .. } => {
                let message = format!("{}: Link to {} is already present!",
                                      link.user_id,
                                      link.linked_user_id);
                let result = matrix_org::post_plain_message(client, access_token, &link.chat_id, message);
                match result {
                    Ok(event_id) => info!("Message posted with event id {}", event_id),
                    Err(error) => error!("Error while sending Matrix message: {:?}", error),
                }
            }
        }
    }

    pub fn report_added_link(&self, client: &Client, link: &UserInfo) {
        match *self {
            Upstream::Matrix { ref access_token, .. } => {
                let message = format!("{}: Link to {} created!", link.user_id, link.linked_user_id);
                let result = matrix_org::post_plain_message(client, access_token, &link.chat_id, message);
                match result {
                    Ok(event_id) => info!("Message posted with event id {}", event_id),
                    Err(error) => error!("Error while sending Matrix message: {:?}", error),
                }
            }
        }
    }
}

impl UserInfo {
    pub fn poll(&mut self, client: &Client) -> Vec<Box<UpdateDesc>> {
        let linked_user_name = self.linked_user_id.to_owned();
        let update_result = self.adapter.poll(client, vec![linked_user_name]);
        let updates = match update_result {
            Err(error) => {
                error!("Error while polling: {}", error.description());
                return Vec::default();
            }
            Ok(updates) => {
                if updates.is_empty() {
                    info!("Nothing found for {}...", self.linked_user_id);
                    return Vec::default();
                }
                updates
            }
        };
        let current_latest_update = updates.iter().map(|u| u.timestamp()).max().unwrap();
        if self.last_update == current_latest_update {
            info!("No updates for {}...", self.linked_user_id);
            return Vec::default();
        }

        // if we have last_update set to zero then this we are newly created user_info
        // in this case, fetch all updates from the adapter and don't report them,
        // instead, skip all the updates and set our timestamp to newest
        if self.last_update.timestamp() == 0 {
            info!("Updating newly created user info timestamp to latest available: {}",
                  current_latest_update);
            self.last_update = current_latest_update;
            return Vec::default();
        }

        let new_updates: Vec<Box<UpdateDesc>> =
            updates.into_iter().filter(|u| u.timestamp() > self.last_update).collect();
        self.last_update = current_latest_update;

        new_updates
    }
}

#[cfg(test)]
mod tests {
    use diesel::sqlite::*;
    use diesel::prelude::*;
    use diesel;
    use entities::*;
    use chrono::prelude::*;
    use database::schema::user_info;

    #[test]
    fn test_insert() {
        let sample = UserInfo {
            id: 0,
            upstream_type: "Matrix".to_owned(),
            chat_id: "chat0".to_owned(),
            user_id: "user0".to_owned(),
            adapter: Adapter::LinuxOrgRu,
            linked_user_id: "user1".to_owned(),
            last_update: NaiveDateTime::from_timestamp(0, 0),
        };
        let conn = SqliteConnection::establish("data/acc-linker-bot.db").expect("Error connecting to sqlite3 db!");

        diesel::insert(&sample)
            .into(user_info::table)
            .execute(&conn)
            .expect("Error saving new post");

    }

}
