extern crate actix_web;
extern crate actix_form_data;

use crate::dashboard::{dashboard_page, DashBoard, dashboard_page_req, file_sender, upload_index, uploader, dashboard_reload_templates};
use crate::database::get_random_token;
use crate::config::Config;
use crate::file_cache::FileCache;

use cookie::Cookie;
use actix_web::{App, HttpResponse, HttpServer, middleware, web, cookie, HttpRequest};
use log::{debug, error};
use actix_web::{Error, http};
use rustls_pemfile;
use rustls::server::ServerConfig;
use rustls::{Certificate, PrivateKey};
use secstr::SecStr;

use std::sync::{Arc, Mutex};
use std::fs::File;
use std::io::BufReader;

pub const AUTH_COOKIE: &'static str = "authid";

fn get_static_file(info: &str, mdata: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    let static_str = match mdata.get_ref().clone().get_str_file(info) {
        Ok(res) => res,
        Err(err) => {
            error!("Error on reading static file: {}", err);
            format!("")
        }
    };

    Ok(HttpResponse::Ok().body(format!("{}", static_str)))
}

fn get_static_file_raw(info: &str, mdata: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    let static_bytes = match mdata.get_ref().clone().get_byte_file(info) {
        Ok(res) => res,
        Err(err) => {
            error!("Error on reading static file: {}", err);
            return Ok(HttpResponse::InternalServerError().body(format!("Error on loading raw file")));
        }
    };

    Ok(HttpResponse::Ok().body(static_bytes))
}

async fn responce_static_file_raw(info: web::Path<String>, mdata: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    get_static_file_raw(&info.into_inner(), mdata)
}

async fn responce_static_file(info: web::Path<String>, mdata: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    get_static_file(&info.into_inner(), mdata)
}

/// Info, used in the auth form when logging in
#[derive(Deserialize)]
struct LoginInfo {
    username: String,
    password: String,
}


/// Handles login requests when LoginInfo has been already sent
async fn login_handler(req: HttpRequest, form: web::Form<LoginInfo>, mdata: web::Data<DashBoard<'_>>, f_cache: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    debug!("login_handler: {:?}", req.cookie(AUTH_COOKIE));

    let nick = form.username.clone();
    let password = SecStr::from(form.password.as_str());

    let validated = match mdata.database.validate_user(&nick, &password) {
        Ok(data) => data,
        Err(e) => {
            error!("Error on handling login: {}", e);
            return Ok(HttpResponse::InternalServerError().body(format!("Error on login")));
        }
    };

    if !validated {
        return Ok(HttpResponse::Ok().body("Incorrect login or password"));
    }

    let token = get_random_token();
    let cookie = Cookie::new(AUTH_COOKIE, &token);

    match mdata.database.assign_cookie(&nick, &token) {
        Ok(_) => { debug!("New login: `{}` -> `{}`", nick, token) }
        Err(e) => {
            error!("Error on assigning cookies: {}", e);
        }
    };

    get_static_file("login_success.html", f_cache).map(|mut resp| {
        resp.add_cookie(&cookie).map_err(|err| {
            error!("Error in adding cookies: {:?}", err);
        }).unwrap_or(());
        resp
    })
}

async fn logout_handler(req: HttpRequest, mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    let cookie = match req.cookie(AUTH_COOKIE) {
        Some(data) => data.value().to_string(),
        None => return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish()),
    };

    match mdata.database.remove_cookie(&cookie) {
        Ok(_) => { debug!("Logout {}", &cookie) }
        Err(e) => {
            error!("Error on removing cookies: {}", e);
        }
    };

    Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/main")).finish())
}

/// Returns standard login page with form for signing in
async fn login_page(mdata: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    get_static_file("login.html", mdata)
}

/// Returns basic main page (currently there is only one button)
async fn main_page(mdata: web::Data<FileCache>) -> Result<HttpResponse, Error> {
    get_static_file("main.html", mdata)
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
    let ds = match DashBoard::new(&config) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on starting the server (make dashboard): {:?}", e);
            return;
        }
    };
    let stat_files = FileCache::new();

    let cert_file = &mut BufReader::new(File::open("cert.pem").unwrap());
    let key_file = &mut BufReader::new(File::open("key.pem").unwrap());

    let cert_chain = rustls_pemfile::certs(cert_file).unwrap();
    let keys = rustls_pemfile::rsa_private_keys(key_file).unwrap();

    let config_tls = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(vec![Certificate { 0: cert_chain[0].clone() }], PrivateKey { 0: keys[0].clone() })
        .unwrap();

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::DefaultHeaders::new().add(("X-Version", "0.2")))
            .wrap(middleware::Logger::new("%T sec  from %a `%r` -> `%s` %b `%{Referer}i` `%{User-Agent}i`"))
            .wrap(middleware::Compress::default())
            .app_data(web::Data::new(ds.clone()))
            .app_data(web::Data::new(stat_files.clone()))
            .service(web::resource("/main").to(main_page))
            .service(web::resource("/").to(main_page))
            .service(web::resource("/login").to(login_page))
            .service(web::resource("/reload").to(dashboard_reload_templates))
            .service(web::resource("/logout").to(logout_handler))
            .service(web::resource("/get_logged_in").route(web::post().to(login_handler)))
            .service(web::resource("/dashboard/{device}")
                .route(web::post().to(dashboard_page_req))
                .route(web::get().to(dashboard_page)))
            .service(web::resource("/dashboard/{device}").to(dashboard_page))
            .service(web::resource("/static/{path}").to(responce_static_file))
            .service(web::resource("/rstatic/{path}").to(responce_static_file_raw))
            .service(web::resource("/download/{path}").to(file_sender))
            .service(
                web::resource("/upload/{path}")
                    .route(web::get().to(upload_index))
                    .route(web::post().to(uploader)),
            )
    })
        .bind_rustls(config.bind_address, config_tls)
        .unwrap()
        .run().await.unwrap();
}