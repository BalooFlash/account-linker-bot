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

const CHALLENGE: &str = "I love lor-bot!";

/// Common errors for application
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

/// Different markdown types for different upstreams
pub enum MarkdownType {
    GitHub,
    Matrix,
    Telegram,
}

/// command syntax is e.g.:
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

/// Update description, provides timestamp when update happened and various ways to
/// represent it in upstreams.
pub trait UpdateDesc {
    fn as_string(&self) -> String;
    fn as_markdown(&self, md_type: MarkdownType) -> String;
    fn as_html(&self) -> String;
    fn timestamp(&self) -> NaiveDateTime;
}


/// Where do we request updates to be sent to
/// and from where do we connect to link accounts
pub trait Upstream {
    /// Connect using credentials provided in config
    fn connect(&mut self, client: &Client, cfg: &Config);

    /// Check updates that this upstream may have and return them
    fn check_updates(&mut self, client: &Client) -> Result<Vec<UpstreamUpdate>>;

    /// Push formatted update from downstream adapter to this upstream
    fn push_update(&self, client: &Client, chat_id: &str, update: Box<UpdateDesc>);

    /// User already requested this link or it already verified, report it
    fn report_duplicate_link(&self, client: &Client, link: UserInfo);

    /// User requested this link, we should verify it in respective downstream, say that to user
    fn report_link_to_verify(&self, client: &Client, link: &UserInfo);

    /// User successfully verified this link, say that
    fn report_added_link(&self, client: &Client, link: &UserInfo);
}

#[derive(Default)]
pub struct Matrix {
    access_token: String,
    last_batch: String,
}

impl Upstream for Matrix {

    fn connect(&mut self, client: &Client, cfg: &Config) {
        if self.access_token.is_empty() {
            self.access_token = matrix_org::connect(client, cfg).unwrap_or_default()
        }
    }

    fn check_updates(&mut self, client: &Client) -> Result<Vec<UpstreamUpdate>> {
        matrix_org::process_updates(client, &self.access_token, &mut self.last_batch)
    }

    fn push_update(&self, client: &Client, chat_id: &str, update: Box<UpdateDesc>) {
        let result = matrix_org::post_update(client, &self.access_token, chat_id, update);
        match result {
            Ok(event_id) => info!("Message posted with event id {}", event_id),
            Err(error) => error!("Error while sending Matrix message: {:?}", error),
        }
    }

    fn report_duplicate_link(&self, client: &Client, link: UserInfo) {
        let display_name = matrix_org::get_display_name(client, &link.user_id).unwrap_or(link.user_id.to_owned());
        let message = format!("{}: Link to {} is already present!", display_name, link.linked_user_id);
        let result = matrix_org::post_plain_message(client, &self.access_token, &link.chat_id, message);
        match result {
            Ok(event_id) => info!("Message posted with event id {}", event_id),
            Err(error) => error!("Error while sending Matrix message: {:?}", error),
        }
    }

    fn report_link_to_verify(&self, client: &Client, link: &UserInfo) {
        let display_name = matrix_org::get_display_name(client, &link.user_id).unwrap_or(link.user_id.to_owned());
        let message = format!("{}: You should prove it's you! Write '{}' without quotes in {}!", display_name, CHALLENGE, link.adapter.to_string());
        let result = matrix_org::post_plain_message(client, &self.access_token, &link.chat_id, message);
        match result {
            Ok(event_id) => info!("Message posted with event id {}", event_id),
            Err(error) => error!("Error while sending Matrix message: {:?}", error),
        }
    }

    fn report_added_link(&self, client: &Client, link: &UserInfo) {
        let display_name = matrix_org::get_display_name(client, &link.user_id).unwrap_or(link.user_id.to_owned());
        let message = format!("{}: Link to {} created!", display_name, link.linked_user_id);
        let result = matrix_org::post_plain_message(client, &self.access_token, &link.chat_id, message);
        match result {
            Ok(event_id) => info!("Message posted with event id {}", event_id),
            Err(error) => error!("Error while sending Matrix message: {:?}", error),
        }
    }
}

/// Downstream where we retrieve updates from
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
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

/// Diesel-related
impl<'a> AsExpression<Text> for &'a Adapter {
    type Expression = AsExprOf<String, Text>;
    fn as_expression(self) -> Self::Expression {
        <String as AsExpression<Text>>::as_expression(self.to_string())
    }
}

/// Diesel-related
impl FromSqlRow<Text, Sqlite> for Adapter {
    fn build_from_row<R: Row<Sqlite>>(row: &mut R) -> result::Result<Self, Box<Error + Send + Sync>> {
        let raw = <String as FromSqlRow<Text, Sqlite>>::build_from_row(row)?;
        let adapter: Result<Adapter> = str::parse(&raw);
        adapter.or_else(|e| Err(e.description().into()))
    }
}


impl Adapter {

    /// Poll data from this downstream adapter. This doesn't usually require any auth
    /// as you don't want to report your non-public posts to chats in upstreams
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

/// User info struct, which provides a link between Connector and Adapter
/// UserInfo struct instances are meant to be alive almost the same amount of time
/// the application is running.
#[derive(Debug, Hash, Eq, Queryable)]
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
    /// Verified link with account or not
    pub verified: bool,
}

/// Diesel-requred insert helper
#[derive(Insertable)]
#[table_name = "user_info"]
pub struct NewUserInfo {
    pub upstream_type: String,
    pub chat_id: String,
    pub user_id: String,
    pub adapter: Adapter,
    pub linked_user_id: String,
    pub last_update: NaiveDateTime,
}

impl PartialEq for UserInfo {
    /// We don't compare internal ids and last_update times
    fn eq(&self, rhs: &UserInfo) -> bool {
        self.chat_id == rhs.chat_id && self.user_id == rhs.user_id && self.linked_user_id == rhs.linked_user_id &&
        self.upstream_type == rhs.upstream_type && self.adapter == rhs.adapter
    }
}

impl UserInfo {

    /// Retrieve info from adapter and update self from that info
    /// * Don't report initial data, report only updates after that
    /// * If the message contains 'I love lor-bot!' then mark self as verified
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

        // try to lookup proof message in adapter
        if !self.verified {
            self.verified = updates.iter().any(|u| u.as_string().contains(CHALLENGE));
        }

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

        // we got updates since last times, return them and update our last known timestamp
        let new_updates: Vec<Box<UpdateDesc>> =
            updates.into_iter().filter(|u| u.timestamp() > self.last_update).collect();
        self.last_update = current_latest_update;

        info!("Found {} updates for {}", new_updates.len(), self.linked_user_id);
        new_updates
    }
}