extern crate actix_identity;
extern crate actix_web;
extern crate form_data;

use std::collections::HashMap;

use actix_identity::Identity;
use actix_web::{Error, HttpResponse, web};
use diesel::r2d2::{self, ConnectionManager};
use diesel::SqliteConnection;
use futures::future::{err, Future, ok};
use futures::IntoFuture;

use crate::database::{get_connection, get_user_devices, get_user_from_cookie};

use self::actix_web::http;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;


fn get_available_devices(pool: &Pool, mapped_devices: &HashMap<String, String>, username: &str) -> String {
    let devices: Vec<String> = match get_user_devices(pool, mapped_devices, username) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in get_available_devices: {:?}", e);
            return "Error on loading".to_string();
        }
    };

    format!("<ul class=\"devlist\">
        <li class=\"devitem\">
            {}
        </li>
    </ul>", devices.iter().map(|x| format!("<a href=\"/{}\">{}</a>", x, x)).collect::<Vec<String>>().join("</li>\n<li class=\"devitem\">"))
}

fn get_available_info(pool: &Pool, username: &str, device: &str) -> String {
    format!("")
}

pub fn dashboard_page(id: Identity, info: web::Path<(String)>, database_url: &String, mapped_devices: &HashMap<String, String>) -> impl Future<Item=HttpResponse, Error=Error> {
    println!("{:?}", id.identity());

    let cookie = match id.identity() {
        Some(data) => data,
        None => return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let conn = match get_connection(database_url) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in dashboard_page at getting the connection: {:?}", e);
            return ok(HttpResponse::InternalServerError().body(format!("Failed to connect to database", e)));
        }
    };


    let user = match get_user_from_cookie(&conn, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in dashboard_page at getting the user: {:?}", e);
            return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };


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
    ", get_available_devices(&conn, mapped_devices, &user), get_available_info(&conn, &user, info.as_str()))))
}