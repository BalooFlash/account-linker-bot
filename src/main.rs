#[macro_use]
extern crate diesel;

#[macro_use]
extern crate log;
extern crate log4rs;

extern crate reqwest;
extern crate select;

extern crate config;

#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate derive_error;
extern crate crossbeam;
extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate chrono;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use reqwest::Client;
use chrono::prelude::*;

use config::Config;
use config::File;

use std::thread;
use std::time::Duration;
use std::path::Path;
use std::fs::create_dir;
use std::error::Error;

mod entities;
mod modules;

use entities::*;

#[cfg(feature = "linux-org-ru")]
use modules::lor_ru;

#[derive(new)]
struct GlobalData {
    conn: SqliteConnection,
    config: Config,
    http_client: Client,
    demands: Vec<UserInfo>,
}

fn main() {
    // init logging
    if Path::new("conf/log4rs.yml").exists() {
        log4rs::init_file("conf/log4rs.yml", Default::default())
            .expect("Must be able to initialize logging!");
    }

    // init database
    if !Path::new("data").exists() {
        debug!("Creating data dir for configs in cwd");
        create_dir("data").expect("Must be able to create data dir!");
    }
    let conn = SqliteConnection::establish("data/acc-linker-bot.db")
        .expect("Error connecting to sqlite3 db!");

    // init HTTP client
    let client = Client::new().expect("Must be able to initialize http client!");

    // init bot configuration
    let mut cfg = Config::new();
    cfg.merge(File::with_name("conf/bot-config.yml"))
        .expect("Must be able to parse config in conf/bot-config.yml");

    // retrieve list of bindings from database
    let mut user_infos = vec![];
    user_infos.push(UserInfo::new(0,
                                  "Kanedias@matrix.org".to_owned(),
                                  "Adonai".to_owned(),
                                  Connector::Matrix,
                                  Adapter::LinuxOrgRu,
                                  Local::now()));

    let app_data = GlobalData::new(conn, cfg, client, user_infos);

    start_event_loop(app_data);
}

fn start_event_loop(data: GlobalData) {
    let http_client = &data.http_client;
    let conf = &data.config;
    loop {
        crossbeam::scope(|scope| for user_info in &data.demands {
            let user_name = user_info.user_name.to_owned();
            scope.spawn(move || {
                user_info.adapter.connect(http_client, conf);
                user_info.connector.connect(http_client, conf);
                let updates = match user_info.adapter.poll(http_client, &vec![user_name]) {
                    Ok(upd) => upd,
                    Err(error) => {
                        error!("Error while polling: {}", error.description());
                        return;
                    }
                };
            });
        });

        debug!("Done polling, sleeping...");
        thread::sleep(Duration::from_millis(3000));
    }
}