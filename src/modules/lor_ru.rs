use std::result::Result;
use std::vec::Vec;
use std::io::Read;

use reqwest::Client;
use select::document::Document;
use select::predicate::{Predicate, Attr, Class, Name};

use chrono::prelude::*;

use modules::UserComment;

const LOR_URL: &'static str = "https://www.linux.org.ru/";

pub struct LorComment {
    common: UserComment,
    post_link: String,
    author_link: String,
}

#[derive(Debug, Error)]
pub enum LorError {
    // Error retrieving LOR HTML page
    HttpError(::reqwest::Error),
    // Error converting response to string
    ConvertError(::std::io::Error)
}

impl ToString for LorComment {

    fn to_string(&self) -> String {
        format!("[{}]({}) posted comment in post [{}]({}): '{}'", self.common.user_name, self.author_link, 
        self.common.post_title, self.post_link, self.common.comment_text)
    }
}

pub fn get_user_posts(user_name: &String, client: &Client) -> Result<Vec<LorComment>, LorError> {
    let url = LOR_URL.to_string() + "search.jsp?range=COMMENTS&sort=DATE&user=" + &user_name;
    let mut body = String::new();
    let mut response = client.get(&url)?.send()?;
    response.read_to_string(&mut body)?;

    let doc = Document::from(body.as_str());
    let mut comments: Vec<LorComment> = vec![];
    for node in doc.find(Name("article").and(Class("msg"))) {
        let (post_link, post_title);
        match node.find(Name("h2").descendant(Name("a"))).next() {
            None => continue,
            Some(post) => {
                post_link = post.attr("href").unwrap_or_default().to_owned();
                post_title = post.text();
            }
        }

        let (author_link, author_name);
        match node.find(Name("a").and(Attr("itemprop", "creator"))).next() {
            None => continue,
            Some(author) => {
                author_link = author.attr("href").unwrap_or_default().to_owned();
                author_name = author.text();
            }
        }

        let (comment_date, comment_text);
        match node.find(Name("time")).next() {
            None => continue,
            Some(time) => {
                let instant = Local::now().with_timezone(Local::now().offset());
                comment_date = time.attr("datetime").map_or(instant,|t| DateTime::parse_from_rfc3339(t).unwrap_or(instant))
            }
        }
        match node.find(Name("div").and(Class("msg_body")).descendant(Name("p"))).next() {
            None => continue,
            Some(text) => {
                comment_text = text.text();
            }
        }

        comments.push(LorComment {
            common: UserComment {
                user_name: author_name,
                post_title: post_title,
                comment_date: comment_date,
                comment_text: comment_text
            },
            post_link: post_link,
            author_link: author_link
        });
    }

    Ok(comments)
}