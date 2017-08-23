#[macro_use] extern crate diesel;
#[macro_use] extern crate log;
extern crate log4rs;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use std::path::Path;
use std::fs::create_dir;

pub mod entities;

fn main() {
    // init logging
    if Path::new("conf/log4rs.yml").exists() {
        log4rs::init_file("conf/log4rs.yml", Default::default()).unwrap();
    }

    // init database
    if !Path::new("data").exists() {
        debug!("Creating data dir for configs in cwd");
        create_dir("data").unwrap();
    }
    let conn = SqliteConnection::establish("data/acc-linker-bot.db").expect("Error connecting to sqlite3 db!");
}