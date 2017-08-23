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

use std::path::Path;
use std::fs::create_dir;
use std::io::{self, Write};

pub mod entities;

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
}