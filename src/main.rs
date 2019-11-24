extern crate webify;

use webify::server::run_server;
use webify::io_tools;
use webify::config;
use std::env;
use std::sync::{Arc, Mutex};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 {
        match args[1].as_str() {
            "--setup" => {
                config::setup();
                return;
            }
            "--uadd" => {
                config::add_user();
                return;
            }
            _ => {
                println!("Unknown argument, exiting");
                return;
            }
        }
    }
    let config = Arc::new(Mutex::new(config::read_config().unwrap()));
    let handler = run_server(config);
}
