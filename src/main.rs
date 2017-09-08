#[macro_use] extern crate diesel;

#[macro_use] extern crate log;
extern crate log4rs;

extern crate reqwest;
extern crate select;

extern crate chrono;

#[macro_use] extern crate derive_new;
#[macro_use] extern crate derive_error;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use reqwest::Client;
use chrono::prelude::*;

use std::thread;
use std::time::Duration;
use std::path::Path;
use std::fs::create_dir;
use std::io::{self, Write};
use std::result::Result;

mod entities;
mod modules;

use entities::*;

#[cfg(feature = "linux-org-ru")]
use modules::lor_ru;

#[derive(new)]
struct GlobalData {
    conn: SqliteConnection,
    http_client: Client,
    demands: Vec<UserInfo>
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

    // init HTTP client
    let client = Client::new().expect("Can't initialize http client!");

    // retrieve list of bindings from database
    let mut user_infos = vec![];
    user_infos.push(UserInfo::new(0, "Kanedias@matrix.org".to_owned(), "Adonai".to_owned(), Connector::Matrix, Adapter::LinuxOrgRu, Local::now()));

    let app_data = GlobalData::new(conn, client, user_infos);

    start_event_loop(app_data);
}

fn start_event_loop(data: GlobalData) {
    loop {
        let http_client = &data.http_client;
        for user_info in &data.demands {
            thread::spawn(|| {
                user_info.adapter.poll(http_client, &vec![user_info.user_name]);
            });
        }

        debug!("Done polling, sleeping...");
        thread::sleep(Duration::from_millis(3000));
    }
}