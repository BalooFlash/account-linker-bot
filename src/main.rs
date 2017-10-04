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
extern crate uuid;
//#[macro_use]
//extern crate lazy_static;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use database::schema::user_info;

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

/*
lazy_static! {
    static ref DB_CONN: Arc<Mutex<SqliteConnection>> = {
        if !Path::new("data").exists() {
            debug!("Creating data dir for configs in cwd");
            create_dir("data").expect("Must be able to create data dir!");
        }
        let conn = SqliteConnection::establish("data/acc-linker-bot.db").expect("Error connecting to sqlite3 db!");
        Arc::new(Mutex::new(conn))
    };
}
*/

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
    let user_infos: Vec<UserInfo> = user_info::table.load(&conn).unwrap();
    info!("Updates: {:?}", user_infos);
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
                    Link(request) => {
                        if data.requests.contains(&request) {
                            // this request was already present, report it
                            upstream.report_duplicate_link(client, request);
                            continue;
                        }
                        upstream.report_link_to_verify(client, &request);
                        data.requests.push(request);
                    }
                    Unlink(user_info) => data.requests.retain(|i| i != &user_info),
                    UnlinkAll { user_name, upstream_type } => {
                        data.requests.retain(|i| i.user_id == user_name && i.upstream_type == upstream_type)
                    }
                }
            }
        }

        for user_info in &mut data.requests {
            let old_verified = user_info.verified;
            let upstream = data.connects.get(&user_info.upstream_type).expect("Must be known upstream type!");
            let updates = user_info.poll(&data.http_client);

            if !user_info.verified {
                // don't report data that wasn't previously verified
                continue
            }


            if !old_verified {
                // this user info just got itself verified, notify and insert to DB
                upstream.report_added_link(client, user_info);

                let new_row = NewUserInfo {
                    upstream_type: user_info.upstream_type.to_owned(),
                    chat_id: user_info.chat_id.to_owned(),
                    user_id: user_info.user_id.to_owned(),
                    adapter: user_info.adapter,
                    linked_user_id: user_info.linked_user_id.to_owned(),
                    last_update: user_info.last_update
                };

                diesel::insert(&new_row)
                    .into(user_info::table)
                    .execute(&data.conn).expect("Error saving new user info!");
            }

            for update in updates {
                upstream.push_update(client, &user_info.chat_id, update);
            }
        }

        debug!("Done polling, sleeping...");
        thread::sleep(Duration::from_millis(3000));
    }
}
