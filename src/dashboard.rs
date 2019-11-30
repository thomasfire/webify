extern crate actix_identity;
extern crate actix_web;
extern crate form_data;

use std::collections::HashMap;

use actix_identity::Identity;
use actix_web::{Error, HttpResponse, web, error};
use diesel::r2d2::{self, ConnectionManager};
use diesel::SqliteConnection;
use futures::future::{err, Future, ok, Either};
use futures::{IntoFuture, Stream};

use actix_multipart::{Field, Multipart, MultipartError};

use crate::database::{get_connection, get_user_devices, get_user_from_cookie, on_init, has_access_to_device, has_access_to_group};
use crate::root_device::RootDev;
use crate::device_trait::*;
use std::io;
use self::actix_web::http;
use std::sync::{Arc, Mutex};
use crate::file_device::FileDevice;
use std::cell::Cell;
use std::fs;
use std::io::Write;
use self::actix_web::http::header::ContentDisposition;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

trait Device: DeviceRead + DeviceWrite + DeviceConfirm + DeviceRequest {}

impl<T> Device for T where T: DeviceRead + DeviceWrite + DeviceConfirm + DeviceRequest {}

#[derive(Deserialize)]
pub struct QCommand {
    pub qtype: char,
    pub group: String,
    pub username: String,
    pub command: String,
    pub payload: String,
}


#[derive(Clone)]
struct Dispatch {
    file_device: FileDevice,
    root_device: RootDev,
}

impl Dispatch {
    pub fn new(conn: Pool) -> Dispatch {
        Dispatch { file_device: FileDevice::new(&conn), root_device: RootDev::new(&conn) }
    }

    pub fn resolve_by_name(&self, devname: &str) -> Result<&dyn Device, String> {
        match devname {
            "filer" => Ok(&self.file_device),
            "root" => Ok(&self.root_device),
            _ => Err("No such device".to_string())
        }
    }
}


#[derive(Clone)]
pub struct DashBoard {
    pub mapped_devices: HashMap<String, String>,
    pub database_url: String,
    pub connections: Pool,
    dispatcher: Dispatch,
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
            dispatcher: Dispatch::new(conn.clone()),
        };
        Ok(ds)
    }

    pub fn dispatch(&self, username: &str, device: &str, query: QCommand) -> Result<String, String> {
        if username != query.username {
            return Err(format!("Wrong command credentials"));
        }

        let daccess = match has_access_to_device(&self.connections, &self.mapped_devices, username, device) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error on dispatching (getting access to dev): {}", e);
                return Err("Error on dispatching".to_string());
            }
        };

        let gaccess = match has_access_to_group(&self.connections, username, &query.group) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Error on dispatching (getting access to group): {}", e);
                return Err("Error on dispatching".to_string());
            }
        };

        if !gaccess || !daccess {
            return Err(format!("User {} has no access to the {}.{}. Contact the admin.", username, device, &query.group));
        }

        let fdevice = match self.dispatcher.resolve_by_name(device) {
            Ok(d) => d,
            Err(e) => return Err(format!("Error at getting the device `{}`: {}", device, e)),
        };

        match query.qtype {
            'R' => fdevice.read_data(&query),
            'W' => fdevice.write_data(&query),
            'Q' => fdevice.request_query(&query),
            'C' => fdevice.confirm_query(&query),
            'D' => fdevice.dismiss_query(&query),
            'S' => fdevice.read_status(),
            _ => Err(format!("Unknown type of the query: {}", query.qtype))
        }
    }

    pub fn get_file_from_filer(&self, username: &str, path: &str) -> Result<Vec<u8>, String> {
        let device = &self.dispatcher.file_device;
        device.get_file(username, path)
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

pub fn dashboard_page_req(id: Identity, info: web::Path<(String)>,
                          form: web::Form<QCommand>, mdata: web::Data<DashBoard>) -> impl Future<Item=HttpResponse, Error=Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in dashboard_page_req at getting the user: {:?}", e);
            return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };


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
                                       , match mdata.dispatch(&user, info.as_str(), form.0) {
            Ok(d) => d,
            Err(e) => e
        })))
}

pub fn file_sender(id: Identity, info: web::Path<(String)>, mdata: web::Data<DashBoard>) -> impl Future<Item=HttpResponse, Error=Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in file_sender at getting the user: {:?}", e);
            return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };

    let file_data = match mdata.get_file_from_filer(&user, info.as_str()) {
        Ok(d) => d,
        Err(e) => return ok(HttpResponse::BadRequest().body(format!("<html>
        <link rel=\"stylesheet\" type=\"text/css\" href=\"lite.css\" media=\"screen\" />\
        <body>
            <p class=\"error\">
        Error on getting the file: {}
    </p>
        </body>
        </html>", e)))
    };


    ok(HttpResponse::Ok().set_header(http::header::CONTENT_TYPE, "text/plain; charset=UTF-8\r\n")
        .set_header(http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"\r\n", info.as_str().split("/").collect::<Vec<&str>>().pop().unwrap_or("some_file")))
        .body(file_data))
}


pub fn upload_index(id: Identity, mdata: web::Data<DashBoard>, info: web::Path<(String)>) -> impl Future<Item=HttpResponse, Error=Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in upload_index at getting the user: {:?}", e);
            return ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };

    let gaccess = match has_access_to_group(&mdata.connections, &user, "filer_write") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error on dispatching (getting access to group): {}", e);
            return ok(HttpResponse::InternalServerError().body("Error on dispatching".to_string()));
        }
    };

    if !gaccess {
        return ok(HttpResponse::Forbidden().body("You are not allowed to upload files"));
    }

    ok(HttpResponse::Ok().body(format!(r#"<html>
        <head><title>Upload to Filer</title></head>
        <link rel="stylesheet" type="text/css" href="lite.css" media="screen" />
        <body>
            <div class="uploader">
                <form target="/{}" method="post" enctype="multipart/form-data">
                    <input type="file" name="file"/>
                    <input type="submit" value="Submit"></button>
                </form>
            </div>
        </body>
    </html>"#, info.as_str())))
}


fn save_file(field: Field, username: String, path: String, mdata: DashBoard) -> impl Future<Item=(), Error=Error> {
    let file_path_string = match field.content_disposition() {
        Some(c_d) => match c_d.get_filename() {
            Some(filename) => filename.replace(' ', "_").to_string(),
            None => return Either::A(err(error::ErrorBadRequest("No content-disposition")))
        },
        None => return Either::A(err(error::ErrorBadRequest("No content-disposition")))
    };
    Either::B(
        field
            .fold((0i64), |(acc), bytes| {
                web::block(|| {
                    mdata.dispatcher.file_device.write_file(&username, &path, &bytes.to_vec()).map_err(|e| {
                        eprintln!("file.write_all failed: {:?}", e);
                        MultipartError::Payload(error::PayloadError::Io(io::Error::new(io::ErrorKind::Other, e)))
                    })?;
                    Ok((0i64))
                })
                    .map_err(|e: error::BlockingError<MultipartError>| {
                        match e {
                            error::BlockingError::Error(e) => e,
                            error::BlockingError::Canceled => MultipartError::Incomplete,
                        }
                    })
            })
            .map(|_acc| ())
            .map_err(|e| {
                eprintln!("save_file failed, {:?}", e);
                err(error::ErrorInternalServerError(format!("{:?}", e)))
            }),
    )
}

pub fn uploader(id: Identity, multipart: Multipart, mdata: web::Data<DashBoard>, info: web::Path<(String)>) -> impl Future<Item=HttpResponse, Error=Error> {


    multipart
        .map_err(error::ErrorInternalServerError)
        .map(|field| {
            let cookie = match id.identity() {
                Some(data) => data,
                None => return Either::A(err(error::ErrorBadRequest("No content-disposition")).into_future()),
            };

            let user = match get_user_from_cookie(&mdata.connections, &cookie) {
                Ok(data) => data,
                Err(e) => {
                    eprintln!("Error in uploader at getting the user: {:?}", e);
                    return Either::A(err(error::ErrorBadRequest("No content-disposition")).into_future());
                }
            };

            let gaccess = match has_access_to_group(&mdata.connections, &user, "filer_write") {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Error on uploader (getting access to group): {}", e);
                    return Either::A(err(error::ErrorBadRequest("No content-disposition")).into_future());
                }
            };

            if !gaccess {
                return Either::A(err(error::ErrorBadRequest("No content-disposition")).into_future());
            }

            return save_file(field, user, info.to_string(), mdata.get_ref().clone());
        })
}