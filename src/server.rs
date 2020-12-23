extern crate actix_web;
extern crate actix_form_data;

use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::BufReader;

use actix_identity::{CookieIdentityPolicy, IdentityService};
use actix_identity::Identity;
use actix_web::{App, HttpResponse, HttpServer, middleware, web};

use crate::config::Config;
use crate::io_tools::read_str;
use crate::file_cache::FileCache;

use self::actix_web::{Error, http};
use crate::dashboard::{dashboard_page, DashBoard, dashboard_page_req, file_sender, upload_index, uploader};
use crate::database::{validate_user, get_random_token, assign_cookie, remove_cookie};
use rustls::internal::pemfile::{certs, rsa_private_keys};
use rustls::{NoClientAuth, ServerConfig};

/// Returns the contents of styles.css
async fn get_styles() -> Result<HttpResponse, Error> {
    let styles_str = match read_str("styles/styles.css") {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error on reading styles: {}", err);
            format!("")
        }
    };

    Ok(HttpResponse::Ok().body(format!("{}", styles_str)))
}

/// Returns the contents of lite.css
async fn get_lite_styles() -> Result<HttpResponse, Error> {
    let styles_str = match read_str("styles/lite.css") {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error on reading styles: {}", err);
            format!("")
        }
    };

    Ok(HttpResponse::Ok().body(format!("{}", styles_str)))
}

async fn get_static_file(info: web::Path<String>, mdata: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    let static_str = match mdata.get_ref().clone().get_str_file(&info.0) {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error on reading static file: {}", err);
            format!("")
        }
    };

    Ok(HttpResponse::Ok().body(format!("{}", static_str)))
}

/// Returns the contents of dashboard.css
async fn get_dash_styles() -> Result<HttpResponse, Error> {
    let styles_str = match read_str("styles/dashboard.css") {
        Ok(res) => res,
        Err(err) => {
            eprintln!("Error on reading styles: {}", err);
            format!("")
        }
    };

    Ok(HttpResponse::Ok().body(format!("{}", styles_str)))
}

/// Info, used in the auth form when logging in
#[derive(Deserialize)]
struct LoginInfo {
    username: String,
    password: String,
}


/// Handles login requests when LoginInfo has been already sent
async fn login_handler(form: web::Form<LoginInfo>, id: Identity, mdata: web::Data<DashBoard>) -> Result<HttpResponse, Error> {
    println!("login_handler: {:?}", id.identity());

    let nick = form.username.clone();
    let password = form.password.clone();

    let validated = match validate_user(&mdata.connections, &nick, &password) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on handling login: {}", e);
            return Ok(HttpResponse::InternalServerError().body(format!("Error on login")));
        }
    };

    if !validated {
        return Ok(HttpResponse::Ok().body("Incorrect login or password"));
    }

    let token = get_random_token();
    id.remember(token.clone());

    match assign_cookie(&mdata.connections, &nick, &token) {
        Ok(_) => { println!("New login") }
        Err(e) => {
            eprintln!("Error on assigning cookies: {}", e);
        }
    };

    Ok(HttpResponse::Ok().body(format!("
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

async fn logout_handler(id: Identity, mdata: web::Data<DashBoard>) -> Result<HttpResponse, Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    id.forget();

    match remove_cookie(&mdata.connections, &cookie) {
        Ok(_) => { println!("Logout") }
        Err(e) => {
            eprintln!("Error on removing cookies: {}", e);
        }
    };

    Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/main").finish())
}

/// Returns standard login page with form for signing in
async fn login_page(_id: Identity) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().body(format!("\
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
async fn main_page() -> Result<HttpResponse, Error> {
    Ok(HttpResponse::Ok().body(format!("
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
#[actix_rt::main]
pub async fn run_server(a_config: Arc<Mutex<Config>>) {
    let config: Config = { a_config.lock().unwrap().clone() };
    let ds = match DashBoard::new(config.db_config) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on starting the server (make dashboard): {:?}", e);
            return;
        }
    };
    let stat_files = FileCache::new();

    let mut config_tls = ServerConfig::new(NoClientAuth::new());
    let cert_file = &mut BufReader::new(File::open("cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("key.pem").unwrap());

    let cert_chain = certs(cert_file).unwrap();
    let mut keys = rsa_private_keys(key_file).unwrap();

    config_tls.set_single_cert(cert_chain, keys.remove(0)).unwrap();


    HttpServer::new(move || {
        App::new()
            .wrap(IdentityService::new(
                CookieIdentityPolicy::new(&[0; 32])
                    .name("auth-id")
                    .secure(false),
            ))
            // enable logger - always register actix-web Logger middleware last
            .wrap(middleware::Logger::default())
            .data(ds.clone())
            .data(stat_files.clone())
            .service(web::resource("/main").to(main_page))
            .service(web::resource("/").to(main_page))
            .service(web::resource("/login").to(login_page))
            .service(web::resource("/logout").to(logout_handler))
            .service(web::resource("/get_logged_in").route(web::post().to(login_handler)))
            .service(web::resource("/dashboard/{device}")
                .route(web::post().to(dashboard_page_req))
                .route(web::get().to(dashboard_page)))
            .service(web::resource("/dashboard/{device}").to(dashboard_page))
            .service(web::resource("/styles.css").to(get_styles))
            .service(web::resource("/lite.css").to(get_lite_styles))
            .service(web::resource("/static/{path}").to(get_static_file))
            .service(web::resource("/dashboard.css").to(get_dash_styles))
            .service(web::resource("/download/{path}").to(file_sender))
            .service(
                web::resource("/upload/{path}")
                    .route(web::get().to(upload_index))
                    .route(web::post().to(uploader)),
            )
    }
    )
        .bind_rustls(config.bind_address, config_tls)
        .unwrap()
        .run().await.unwrap();
}