#[macro_use]
extern crate diesel;

#[macro_use]
extern crate log;
extern crate log4rs;

extern crate reqwest;
extern crate select;

extern crate config;

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate derive_error;
extern crate crossbeam;
extern crate chrono;
#[macro_use]
extern crate itertools;
extern crate regex;
extern crate uuid;

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
use std::collections::HashMap;

mod entities;
mod modules;

use entities::*;
use entities::UpstreamUpdate::*;

#[derive(new)]
struct GlobalData {
    conn: SqliteConnection,
    config: Config,
    http_client: Client,
    connects: HashMap<String, Connector>,
    requests: Vec<UserInfo>,
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
    user_infos.push(UserInfo::new(0, "0".to_owned(),
                                  "Kanedias@matrix.org".to_owned(),
                                  "Adonai".to_owned(),
                                  "Matrix".to_owned(),
                                  Adapter::LinuxOrgRu,
                                  Local::now().with_timezone(Local::now().offset())));

    let mut app_data = GlobalData::new(conn, cfg, client, HashMap::new(), user_infos);
    app_data.connects.insert("Matrix".to_owned(),
                             Connector::Matrix { access_token: String::default(), last_batch: String::default() });

    start_event_loop(app_data);
}

fn start_event_loop(mut data: GlobalData) {
    let client = &data.http_client;
    loop {
        // connect all upstreams and process invites/leaves etc.
        for upstream in data.connects.values_mut() {
            upstream.connect(client, &data.config);
            let new_demands = upstream.check_updates(client);
            let demands = match new_demands {
                Err(error) => {
                    error!("Couldn't retrieve updates from upstream: {}", error);
                    continue;
                },
                Ok(demands) => demands,
            };

            for d in demands {
                match d {
                    Link(new_user_info) => data.requests.push(new_user_info),
                    Unlink {..} => {},
                }
            }
        }

        for user_info in data.requests.iter_mut() {
            let connector = data.connects.get(&user_info.connector_type).expect("Must be known connector type!");
            let updates = user_info.poll(&data.http_client);
            for update in updates {
                connector.push(client, user_info, update);
            }
        }

        debug!("Done polling, sleeping...");
        thread::sleep(Duration::from_millis(3000));
    }
}