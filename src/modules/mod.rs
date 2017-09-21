extern crate chrono;

use self::chrono::prelude::*;
use entities::UpdateDesc;
use entities::MarkdownType;

#[cfg(feature = "linux-org-ru")]
pub mod lor_ru;

pub mod matrix_org;

pub struct UserComment {
    user_name: String,
    post_title: String,
    comment_date: DateTime<FixedOffset>,
    comment_text: String,
}

impl UpdateDesc for UserComment {
    fn as_string(&self) -> String {
        format!(
            "{}: {} added comment to post {}:\n\t'{}'",
            self.comment_date.to_rfc3339(),
            self.post_title,
            self.user_name,
            self.comment_text
        )
    }

    fn as_markdown(&self, md_type: MarkdownType) -> String {
        self.as_string()
    }

    fn timestamp(&self) -> DateTime<FixedOffset> {
        self.comment_date
    }
}