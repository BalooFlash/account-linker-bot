use std::result::Result;
use std::vec::Vec;

use reqwest::Client;
use reqwest::Error;
use reqwest::Url;
use select::document::Document;
use select::predicate::{Predicate, Attr, Class, Name};

use modules::UserComment;

const LOR_URL: &'static str = "https://www.linux.org.ru/";

struct LorComment {
    common: UserComment,
    postLink: String,
    authorLink: String,
}


pub fn get_user_posts(userName: String, client: &Client) -> Result<Vec<LorComment>, StdError> {
    let url = LOR_URL.to_string() + "search.jsp?range=COMMENTS&sort=DATE&user=" + &userName;
    let mut body = String::new();
    let response = client.get(&url)?.send()?;

    let doc = Document::from(body.as_str());
    return res;
}