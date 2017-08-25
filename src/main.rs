#[macro_use] extern crate diesel;

#[macro_use] extern crate log;
extern crate log4rs;

extern crate futures;
extern crate tokio_core;
extern crate hyper;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;


use futures::{Future, Stream};
use tokio_core::reactor::Core;
use hyper::Client;

use std::thread;
use std::time::Duration;
use std::path::Path;
use std::fs::create_dir;
use std::io::{self, Write};
use std::result::Result;

mod entities;
mod modules;

struct GlobalData<T> {
    conn: SqliteConnection,
    httpClient: Client<T>
}

impl<T> GlobalData<T> {
    pub fn new(dbConn: SqliteConnection, httpClient: Client<T>) -> GlobalData<T> {
        GlobalData { conn: dbConn, httpClient: httpClient }
    }
}

fn main() {
    // init logging
    if Path::new("conf/log4rs.yml").exists() {
        log4rs::init_file("conf/log4rs.yml", Default::default()).expect("Failed to initialize logging!");
    }

    // init database
    if !Path::new("data").exists() {
        debug!("Creating data dir for configs in cwd");
        create_dir("data").expect("Failed to create data dir!");
    }
    let conn = SqliteConnection::establish("data/acc-linker-bot.db").expect("Error connecting to sqlite3 db!");

    let mut core = Core::new().expect("Failed to init event loop!");
    let client = Client::new(&core.handle());

    let appData = GlobalData::new(conn, client);
    
    start_event_loop(appData);
}

fn start_event_loop<T>(data: GlobalData<T>) -> Result<(), ()> {
    loop {
        debug!("Done polling, sleeping...");
        thread::sleep(Duration::from_millis(100));
    }
}   