extern crate actix_web;
extern crate diesel;

use std::sync::{Arc, Mutex};

use actix_web::{App, HttpServer, Responder, web, HttpResponse};

use crate::config::Config;
use crate::io_tools::read_str;

fn main_page() -> impl Responder {
    HttpResponse::Ok().body(format!("
    <!DOCTYPE html>
    <html>
    <head>
        <title>Webify Main</title>
    </head>
    <body>
    <a href=\"/login\" class=\"login\">Log In</a>
    </body>
    </html>"))
}


pub fn run_server(a_config: Arc<Mutex<Config>>) {
    let config = { a_config.lock().unwrap().clone() };

    match HttpServer::new(|| App::new().service(
        web::resource("/main").to(main_page))
    )
        .bind(config.bind_address)
        .unwrap()
        .run() {
        Ok(_) => println!("Server has been started."),
        Err(err) => eprintln!("Error on starting the server: {:?}", err)
    };
}