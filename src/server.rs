extern crate actix_web;
extern crate form_data;

use std::sync::{Arc, Mutex};

use actix_identity::{CookieIdentityPolicy, IdentityService};
use actix_identity::Identity;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};
use futures::future::{Future, ok};

use crate::config::Config;
use crate::io_tools::read_str;

use self::actix_web::Error;
use crate::dashboard::{dashboard_page, DashBoard, dashboard_page_req, file_sender, upload_index, uploader};
use crate::database::{validate_user, get_random_token, assign_cookie};

/// Returns the contents of styles.css
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

/// Returns the contents of lite.css
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

/// Returns the contents of dashboard.css
fn get_dash_styles() -> impl Future<Item=HttpResponse, Error=Error> {
    let styles_str = match read_str("styles/dashboard.css") {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error on reading styles: {}", err);
            format!("")
        }
    };

    ok(HttpResponse::Ok().body(format!("{}", styles_str)))
}

/// Info, used in the auth form when logging in
#[derive(Deserialize)]
struct LoginInfo {
    username: String,
    password: String,
}


/// Handles login requests when LoginInfo has been already sent
fn login_handler(form: web::Form<LoginInfo>, id: Identity, mdata: web::Data<DashBoard>) -> impl Future<Item=HttpResponse, Error=Error> {
    println!("login_handler: {:?}", id.identity());

    let nick = form.username.clone();
    let password = form.password.clone();

    let validated = match validate_user(&mdata.connections, &nick, &password) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on handling login: {}", e);
            return ok(HttpResponse::InternalServerError().body(format!("Error on login")));
        }
    };

    if !validated {
        return ok(HttpResponse::Ok().body("Incorrect login or password"));
    }

    let token = get_random_token();
    id.remember(token.clone());

    match assign_cookie(&mdata.connections, &nick, &token) {
        Ok(_) => { println!("New login") }
        Err(e) => {
            eprintln!("Error on assigning cookies: {}", e);
        }
    };

    ok(HttpResponse::Ok().body(format!("
    <!DOCTYPE html>
    <html>
    <link rel=\"stylesheet\" type=\"text/css\" href=\"lite.css\" media=\"screen\" />
    <head>
        <title>Webify Main</title>
    </head>
    <body>
    <script type=\"text/JavaScript\">
      setTimeout(\"location.href='/dashboard/filer';\", 1500);
    </script>
    <p class=\"info\">
        Logged in. Redirecting in seconds, if this doesn't help, click here: <a href=\"/dashboard\">Go to Dashboard</a>
    </p>
    </body>
    </html>
    ")))
}

/// Returns standard login page with form for signing in
fn login_page(_id: Identity) -> impl Future<Item=HttpResponse, Error=Error> {
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

/// Returns basic main page (currently there is only one button)
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

/// Runs all initial functions and starts the server.
/// Reference to the mutexed config is needed
///
/// # Example
/// ```rust
/// use std::sync::{Arc, Mutex};
/// use std::thread;
/// use webify::config;
/// use webify::server::run_server;
///
/// let config = Arc::new(Mutex::new(config::read_config::<config::Config>(config::DEFAULT_CONFIG_PATH).unwrap()));
/// let handler = thread::spawn(move || run_server(config));
/// assert!(handler.join().is_ok());
/// ```
pub fn run_server(a_config: Arc<Mutex<Config>>) {
    let config: Config = { a_config.lock().unwrap().clone() };
    let ds = match DashBoard::new(config.db_config) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on starting the server (make dashboard): {:?}", e);
            return;
        }
    };

    match HttpServer::new(move || {
        App::new()
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&[0; 32])
                    .name("auth-id")
                    .secure(false),
            ))
            // enable logger - always register actix-web Logger middleware last
            .wrap(middleware::Logger::default())
            .data(ds.clone())
            .service(web::resource("/main").to_async(main_page))
            .service(web::resource("/").to_async(main_page))
            .service(web::resource("/login").to_async(login_page))
            .service(web::resource("/get_logged_in").route(web::post().to_async(login_handler)))
            .service(web::resource("/dashboard/{device}")
                .route(web::post().to_async(dashboard_page_req))
                .route(web::get().to_async(dashboard_page)))
            .service(web::resource("/dashboard/{device}").to_async(dashboard_page))
            .service(web::resource("/styles.css").to_async(get_styles))
            .service(web::resource("/lite.css").to_async(get_lite_styles))
            .service(web::resource("/dashboard.css").to_async(get_dash_styles))
            .service(web::resource("/download/{path}").to_async(file_sender))
            .service(
                web::resource("/upload/{path}")
                    .route(web::get().to_async(upload_index))
                    .route(web::post().to_async(uploader)),
            )
    }
    )
        .bind(config.bind_address)
        .unwrap()
        .run() {
        Ok(_) => println!("Server has been started."),
        Err(e) => eprintln!("Error on starting the server: {:?}", e)
    };
}