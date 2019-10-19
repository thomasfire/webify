extern crate actix_web;
extern crate form_data;
extern crate actix_identity;

use actix_web::{web, HttpResponse, Error};
use actix_identity::Identity;
use futures::future::{err, Future, ok};
//use crate::database::


fn get_available_devices(username: &str) -> String {
    format!("")
}

fn get_available_info(username: &str, device: &str) -> String {
    format!("")
}

pub fn dashboard_page(id: Identity) -> impl Future<Item=HttpResponse, Error=Error> {
    println!("{:?}", id.identity());

    ok(HttpResponse::Ok().body(format!("
    <!DOCTYPE html>
    <html>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"lite.css\" media=\"screen\" />
    <head>
        <title>Webify Dashboard</title>
    </head>
    <body>
        <div class=\"dashboard\">
            <h2>Dashboard</h2>
            <div class=\"devices\">
                {}
            </div>
            <div class=\"info\">
                {}
            </div>
        </div>
    </body>
    </html>
    ", get_available_devices(""), get_available_info("", ""))))
}