extern crate actix_web;
extern crate form_data;

use std::sync::{Arc, Mutex};

use actix_identity::{CookieIdentityPolicy, IdentityService};
use actix_identity::Identity;
use actix_web::{App, HttpResponse, HttpServer, middleware, Responder, web};
use form_data::Form;
use futures::future::{err, Future, ok};

use crate::config::Config;
use crate::io_tools::read_str;

use self::actix_web::Error;
use crate::dashboard::dashboard_page;

fn get_styles() -> impl Future<Item=HttpResponse, Error=Error> {
    let styles_str = match read_str("styles/styles.css") {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error on reading styles: {}", err);
            format!("")
        }
    };

    ok(HttpResponse::Ok().body(format!("{}", styles_str)))
}

fn get_lite_styles() -> impl Future<Item=HttpResponse, Error=Error> {
    let styles_str = match read_str("styles/lite.css") {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error on reading styles: {}", err);
            format!("")
        }
    };

    ok(HttpResponse::Ok().body(format!("{}", styles_str)))
}


#[derive(Deserialize)]
struct LoginInfo {
    username: String,
    password: String,
}

fn login_handler(form: web::Form<LoginInfo>, id: Identity) -> impl Future<Item=HttpResponse, Error=Error> {
    println!("{:?}", id.identity());

    ok(HttpResponse::Ok().body(format!("
    <!DOCTYPE html>
    <html>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"lite.css\" media=\"screen\" />
    <head>
        <title>Webify Main</title>
    </head>
    <body>
    <script type=\"text/JavaScript\">
      setTimeout(\"location.href='/dashboard';\", 1500);
    </script>
    <p class=\"info\">
        Logged in. Redirecting in seconds, if this doesn't help, click here: <a href=\"/dashboard\">Go to Dashboard</a>
    </p>
    </body>
    </html>
    ")))
}

fn login_page(id: Identity) -> impl Future<Item=HttpResponse, Error=Error> {
    ok(HttpResponse::Ok().body(format!("\
    <!DOCTYPE html>
    <html>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"styles.css\" media=\"screen\" />
    <head>
        <title>Webify Main</title>
    </head>
    <body>
    <div class=\"login_form\">
        <form action=\"/get_logged_in\"  method=\"post\" >
            <div class=\"text_field\">
               Username:<br>
              <input type=\"text\" name=\"username\" value=\"Weber\" class=\"username\">
              <br>
              Password:<br>
              <input type=\"password\" name=\"password\" value=\"123\" class=\"password\">
              <br><br>
            </div>
              <input type=\"submit\" value=\"Log In\" class=\"button\">
        </form>
    </div>
    </body>
    </html>")))
}

fn main_page() -> impl Future<Item=HttpResponse, Error=Error> {
    ok(HttpResponse::Ok().body(format!("
    <!DOCTYPE html>
    <html>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"styles.css\" media=\"screen\" />
    <head>
        <title>Webify Main</title>
    </head>
    <body>
    <div class=\"login_btn\">
        <a href=\"/login\">Log In</a>
    </div>
    </body>
    </html>")))
}


pub fn run_server(a_config: Arc<Mutex<Config>>) {
    let config = { a_config.lock().unwrap().clone() };

    match HttpServer::new(||
        App::new()
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&[0; 32])
                    .name("auth-id")
                    .secure(true),
            ))
            // enable logger - always register actix-web Logger middleware last
            .wrap(middleware::Logger::default())
            .service(web::resource("/main").to_async(main_page))
            .service(web::resource("/").to_async(main_page))
            .service(web::resource("/login").to_async(login_page))
            .service(web::resource("/get_logged_in").route(web::post().to_async(login_handler)))
            .service(web::resource("/dashboard/{device}").to_async(dashboard_page))
            .service(web::resource("/styles.css").to_async(get_styles))
            .service(web::resource("/lite.css").to_async(get_lite_styles))
    )
        .bind(config.bind_address)
        .unwrap()
        .run() {
        Ok(_) => println!("Server has been started."),
        Err(err) => eprintln!("Error on starting the server: {:?}", err)
    };
}