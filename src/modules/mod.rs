extern crate chrono;

use self::chrono::prelude::*;

#[cfg(feature = "linux-org-ru")]
pub mod lor_ru;

pub struct UserComment {
    user_name: String,
    post_title: String,
    comment_date: DateTime<FixedOffset>,
    comment_text: String,
}

impl ToString for UserComment {

    fn to_string(&self) -> String {
        format!("{} posted comment: '{}'", self.user_name, self.comment_text)
    }
}