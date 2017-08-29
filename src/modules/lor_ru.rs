use std::result::Result;
use std::vec::Vec;

use reqwest::Client;
use reqwest::Error;
use reqwest::Url;
use select::document::Document;
use select::predicate::{Predicate, Attr, Class, Name};

use modules::UserComment;

const LOR_URL: &'static str = "https://www.linux.org.ru/";

pub struct LorComment {
    common: UserComment,
    postLink: String,
    authorLink: String,
}


pub fn get_user_posts(userName: String, client: &Client) -> Result<Vec<LorComment>, Error> {
    let url = LOR_URL.to_string() + "search.jsp?range=COMMENTS&sort=DATE&user=" + &userName;
    let mut body = String::new();
    let response = client.get(&url)?.send()?;

    let doc = Document::from(body.as_str());
    let comments: Vec<LorComment> = vec![];
    for node in doc.find(Class("messages")) {
        let question = node.find(Class("question-hyperlink")).next().unwrap();
        let votes = node.find(Class("vote-count-post")).next().unwrap().text();
        let answers = node.find(Class("status").descendant(Name("strong")))
            .next()
            .unwrap()
            .text();
        let tags = node.find(Class("post-tag")).map(|tag| tag.text()).collect::<Vec<_>>();
        let asked_on = node.find(Class("relativetime")).next().unwrap().text();
        let asker = node.find(Class("user-details").descendant(Name("a")))
            .next()
            .unwrap()
            .text();
        println!(" Question: {}", question.text());
        println!("  Answers: {}", answers);
        println!("    Votes: {}", votes);
        println!("   Tagged: {}", tags.join(", "));
        println!(" Asked on: {}", asked_on);
        println!("    Asker: {}", asker);
        println!("Permalink: http://stackoverflow.com{}",
                 question.attr("href").unwrap());
        println!("");
    }

    Ok(comments)
}