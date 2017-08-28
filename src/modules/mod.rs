extern crate chrono;

use self::chrono::prelude::*;

#[cfg(feature = "linux-org-ru")]
pub mod lor_ru;

struct UserComment {
    userName: String,
    postTitle: String,
    commentDate: DateTime<Utc>,
    commentText: String
}