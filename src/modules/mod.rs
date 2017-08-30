extern crate chrono;

use self::chrono::prelude::*;

#[cfg(feature = "linux-org-ru")]
pub mod lor_ru;

struct UserComment {
    user_name: String,
    post_title: String,
    comment_date: DateTime<FixedOffset>,
    comment_text: String,
}