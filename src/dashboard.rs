extern crate actix_identity;
extern crate actix_web;
extern crate form_data;

use actix_identity::Identity;
use actix_web::{Error, HttpResponse, web};
use diesel::r2d2::{self, ConnectionManager};
use diesel::SqliteConnection;
use futures::future::{err, Future, ok};

use crate::database::{get_connection, get_user_devices};

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;


fn get_available_devices(pool: Pool, username: &str) -> String {
    format!("")
}

fn get_available_info(pool: Pool, username: &str, device: &str) -> String {
    format!("")
}

pub fn dashboard_page(id: Identity, database_url: String) -> impl Future<Item=HttpResponse, Error=Error> {
    println!("{:?}", id.identity());

    let conn = match get_connection(database_url) {
        Ok(data) => data,
        Err(err) => {
            eprintln!("Error in dashboard_page at getting the connection: {:?}", err);
            return err(HttpResponse::InternalServerError());
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
    ", get_available_devices(""), get_available_info("", ""))))
}