use chrono::prelude::*;
use entities::UpdateDesc;
use entities::MarkdownType;

#[cfg(feature = "linux-org-ru")]
pub mod lor_ru;
pub mod matrix_org;
pub mod mankier;

/// Simplest generic user comment structure that may be convenient
/// for dumb downstream adapters
pub struct UserComment {
    user_name: String,
    post_title: String,
    comment_date: NaiveDateTime,
    comment_text: String,
}

impl UpdateDesc for UserComment {
    fn as_string(&self) -> String {
        format!("{}: {} added comment to post {}:\n\t'{}'",
                self.comment_date,
                self.post_title,
                self.user_name,
                self.comment_text)
    }

    fn as_markdown(&self, _: MarkdownType) -> String {
        self.as_string()
    }

    fn as_html(&self) -> String {
        self.as_string()
    }

    fn timestamp(&self) -> NaiveDateTime {
        self.comment_date
    }
}
