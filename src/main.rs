#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_codegen;

#[macro_use]
extern crate log;
extern crate log4rs;

extern crate reqwest;
extern crate select;

extern crate config;

extern crate serde_json;
extern crate serde;
#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate derive_new;
#[macro_use]
extern crate derive_error;
extern crate crossbeam;
extern crate chrono;
extern crate regex;
extern crate uuid;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;

use reqwest::Client;

use config::Config;
use config::File;

use std::thread;
use std::time::Duration;
use std::path::Path;
use std::fs::create_dir;
use std::collections::HashMap;

pub mod database;
mod entities;
mod modules;

use entities::*;
use entities::UpstreamUpdate::*;

#[derive(new)]
struct GlobalData {
    conn: SqliteConnection,
    config: Config,
    http_client: Client,
    connects: HashMap<String, Upstream>,
    requests: Vec<UserInfo>,
}

fn main() {
    // init logging
    if Path::new("conf/log4rs.yml").exists() {
        log4rs::init_file("conf/log4rs.yml", Default::default()).expect("Must be able to initialize logging!");
    }

    // init database
    if !Path::new("data").exists() {
        debug!("Creating data dir for configs in cwd");
        create_dir("data").expect("Must be able to create data dir!");
    }
    let conn = SqliteConnection::establish("data/acc-linker-bot.db").expect("Error connecting to sqlite3 db!");

    // init HTTP client
    let client = Client::new().expect("Must be able to initialize http client!");

    // init bot configuration
    let mut cfg = Config::new();
    cfg.merge(File::with_name("conf/bot-config.yml")).expect("Must be able to parse config in conf/bot-config.yml");

    // retrieve list of bindings from database
    let user_infos = vec![];

    let mut app_data = GlobalData::new(conn, cfg, client, HashMap::new(), user_infos);
    app_data.connects.insert("Matrix".to_owned(),
                             Upstream::Matrix {
                                 access_token: String::default(),
                                 last_batch: String::default(),
                             });

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
                    error!("Couldn't retrieve updates from upstream: {:?}", error);
                    continue;
                }
                Ok(demands) => demands,
            };

            for d in demands {
                match d {
                    Link(new_user_info) => {
                        if data.requests.contains(&new_user_info) {
                            // this request was already present, report it
                            upstream.report_duplicate_link(client, new_user_info);
                            continue;
                        }
                        upstream.report_added_link(client, &new_user_info);
                        data.requests.push(new_user_info);
                    }
                    Unlink(user_info) => data.requests.retain(|i| i != &user_info),
                    UnlinkAll { user_name, upstream_type } => {
                        data.requests.retain(|i| i.user_id == user_name && i.upstream_type == upstream_type)
                    }
                }
            }
        }

        for user_info in data.requests.iter_mut() {
            let upstream = data.connects.get(&user_info.upstream_type).expect("Must be known upstream type!");
            let updates = user_info.poll(&data.http_client);
            for update in updates {
                upstream.push_update(client, &user_info.chat_id, update);
            }
        }

        debug!("Done polling, sleeping...");
        thread::sleep(Duration::from_millis(3000));
    }
}
