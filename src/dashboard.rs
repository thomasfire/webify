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

use crate::database::{get_connection, get_user_devices, get_user_from_cookie, on_init};

use self::actix_web::http;
use std::sync::{Arc, Mutex};

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone)]
pub struct DashBoard {
    pub mapped_devices: HashMap<String, String>,
    pub database_url: String,
    pub connections: Pool,
}


impl DashBoard {
    pub fn new(database_url: String) -> Result<DashBoard, String> {
        let conn: Pool = match get_connection(&database_url) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Error in DashBoard::new at getting the connection: {:?}", e);
                return Err(format!("Error in DashBoard::new at getting the connection: {:?}", e));
            }
        };

        let devices: HashMap<String, String> = match on_init(&conn) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Error in DashBoard::new at getting the devices: {:?}", e);
                return Err(format!("Error in DashBoard::new at getting the devices: {:?}", e));
            }
        };

        let ds: DashBoard = DashBoard {
            mapped_devices: devices.clone(),
            database_url: database_url.clone(),
            connections: conn.clone(),
        };
        Ok(ds)
    }
}


fn get_available_devices(pool: &Pool, mapped_devices: &HashMap<String, String>, username: &str) -> String {
    let devices: Vec<String> = match get_user_devices(pool, mapped_devices, username) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in get_available_devices: {:?}", e);
            return "Error on loading".to_string();
        }
    };

    format!("<ul class=\"devlist\">
            {}
    </ul>", devices.iter()
        .map(|x| format!("<li class=\"devitem\"><a href=\"/{}\">{}</a></li>", x, x))
        .collect::<Vec<String>>().join("\n"))
}

fn get_available_info(pool: &Pool, username: &str, device: &str) -> String {
    format!("")
}

pub fn dashboard_page(id: Identity, info: web::Path<(String)>, mdata: web::Data<DashBoard>) -> impl Future<Item=HttpResponse, Error=Error> {
    println!("{:?}", id.identity());

    let cookie = match id.identity() {
        Some(data) => data,
        None => return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in dashboard_page at getting the user: {:?}", e);
            return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };

    println!("user: {}", user);

    ok(HttpResponse::Ok().body(format!("
    <!DOCTYPE html>
    <html>
    <head>
        <title>Webify Dashboard</title>
        <link rel=\"stylesheet\" type=\"text/css\" href=\"../dashboard.css\" media=\"screen\" />
    </head>
    <body>

        <div class=\"dashboard\">
        <h2>Dashboard</h2>
            <div class=\"devices\">
                <div class=\"devrow\">
                    <div class=\"devcell\">
                        Available devices: <br>
                        {}
                    </div>
                </div>
            </div>
            <div class=\"info\">
                <div class=\"inforow\">
                    <div class=\"infocell\">
                        Info: <br>
                        {}
                    </div>
                </div>
            </div>
        </div>
    </body>
    </html>
    ", get_available_devices(&mdata.connections, &mdata.mapped_devices, &user)
                                       , get_available_info(&mdata.connections, &user, info.as_str()))))
}