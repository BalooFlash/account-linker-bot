use std::vec::Vec;
use std::io::Read;

use reqwest::Client;
use select::document::Document;
use select::predicate::{Predicate, Attr, Class, Name};

use chrono::prelude::*;

use modules::UserComment;
use entities::*;

const LOR_URL: &'static str = "https://www.linux.org.ru/";

pub struct LorComment {
    common: UserComment,
    post_link: String,
    author_link: String,
}

impl UpdateDesc for LorComment {
    fn as_string(&self) -> String {
        self.common.as_string()
    }

    fn as_markdown(&self, md_type: MarkdownType) -> String {
        match md_type {
            MarkdownType::Matrix | MarkdownType::GitHub => {
                format!("{}: [{}]({}) added comment to post [{}]({}):\n\t{}",
                        self.common.comment_date,
                        self.common.user_name,
                        self.author_link,
                        self.common.post_title,
                        self.post_link,
                        self.common.comment_text)
            }
            MarkdownType::Telegram => self.as_string(),
        }
    }

    fn as_html(&self) -> String {
        format!("{}: <a href='{}'>{}</a> added comment to post <a href='{}'>{}</a>:<br/>{}",
                self.common.comment_date,
                self.author_link,
                self.common.user_name,
                self.post_link,
                self.common.post_title,
                self.common.comment_text)
    }

    fn timestamp(&self) -> NaiveDateTime {
        self.common.comment_date
    }
}

pub fn get_user_posts(user_name: &String, client: &Client) -> Result<Vec<LorComment>> {
    let url = LOR_URL.to_string() + "search.jsp?range=COMMENTS&sort=DATE&user=" + &user_name;
    let mut body = String::new();
    let mut response = client.get(&url)?.send()?;
    response.read_to_string(&mut body)?;

    let doc = Document::from(body.as_str());
    let mut comments: Vec<LorComment> = vec![];
    for node in doc.find(Name("article").and(Class("msg"))) {
        // extract post data
        let (post_link, post_title) = match node.find(Name("h2").descendant(Name("a"))).next() {
            None => continue,
            Some(post) => {
                let pl = post.attr("href").unwrap_or_default();
                let pt = post.text();
                (pl, pt)
            }
        };

        // extract author data
        let (author_link, author_name) = match node.find(Name("a").and(Attr("itemprop", "creator"))).next() {
            None => continue,
            Some(author) => {
                let al = author.attr("href").unwrap_or_default();
                let an = author.text();
                (al, an)
            }
        };

        // extract comment data
        let (comment_date, comment_text);
        match node.find(Name("time")).next() {
            None => continue,
            Some(time) => {
                let instant = Local::now().with_timezone(Local::now().offset());
                comment_date = time.attr("datetime").map_or(instant,
                                                            |t| DateTime::parse_from_rfc3339(t).unwrap_or(instant))
            }
        }
        match node.find(Name("div").and(Class("msg_body")).descendant(Name("p")))
            .next() {
            None => continue,
            Some(text) => {
                comment_text = text.text();
            }
        }

        comments.push(LorComment {
            common: UserComment {
                user_name: author_name,
                post_title,
                comment_date: comment_date.naive_utc(),
                comment_text,
            },
            post_link: LOR_URL.to_owned() + post_link,
            author_link: LOR_URL.to_owned() + author_link,
        });
    }

    Ok(comments)
}