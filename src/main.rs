#[macro_use]
extern crate diesel;

#[macro_use]
extern crate log;
extern crate log4rs;

extern crate reqwest;
extern crate select;

extern crate chrono;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use reqwest::Client;

use std::thread;
use std::time::Duration;
use std::path::Path;
use std::fs::create_dir;
use std::io::{self, Write};
use std::result::Result;

mod entities;
mod modules;

#[cfg(feature = "linux-org-ru")]
use modules::lor_ru;

struct GlobalData {
    conn: SqliteConnection,
    httpClient: Client,
}

impl GlobalData {
    pub fn new(db_conn: SqliteConnection, httpClient: Client) -> GlobalData {
        GlobalData {
            conn: db_conn,
            httpClient: httpClient,
        }
    }
}

fn main() {
    // init logging
    if Path::new("conf/log4rs.yml").exists() {
        log4rs::init_file("conf/log4rs.yml", Default::default())
            .expect("Failed to initialize logging!");
    }

    // init database
    if !Path::new("data").exists() {
        debug!("Creating data dir for configs in cwd");
        create_dir("data").expect("Failed to create data dir!");
    }
    let conn = SqliteConnection::establish("data/acc-linker-bot.db")
        .expect("Error connecting to sqlite3 db!");

    let client = Client::new().unwrap();

    let app_data = GlobalData::new(conn, client);

    // retrieve list of bindings from database

    start_event_loop(app_data);
}

fn start_event_loop(data: GlobalData) {
    loop {
        #[cfg(feature = "linux-org-ru")]
        lor_ru::get_user_posts("Adonai".to_string(), &data.httpClient);

        debug!("Done polling, sleeping...");
        thread::sleep(Duration::from_millis(300));
    }
}