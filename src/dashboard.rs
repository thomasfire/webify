extern crate actix_identity;
extern crate actix_web;
extern crate actix_form_data;

use crate::database::{get_connection, get_user_devices, get_user_from_cookie, on_init, has_access_to_device, has_access_to_group};
use crate::root_device::RootDev;
use crate::device_trait::*;
use crate::file_device::FileDevice;
use crate::printer_device::PrinterDevice;
use crate::blog_device::BlogDevice;
use crate::config::Config;
use crate::template_cache::TemplateCache;

use actix_identity::Identity;
use actix_web::{Error, HttpResponse, web, error, http};
use diesel::r2d2::{self, ConnectionManager};
use diesel::SqliteConnection;
use futures::StreamExt;
use serde_json::Value as jsVal;
use serde_json::json;
use actix_multipart::Multipart;

use std::sync::Arc;
use std::collections::{HashMap, BTreeMap};

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

trait Device: DeviceRead + DeviceWrite + DeviceConfirm + DeviceRequest {}

impl<T> Device for T where T: DeviceRead + DeviceWrite + DeviceConfirm + DeviceRequest {}


/// Here we must tell you how requests are handled.
///
/// First of all, every request to the device should be formed as QCommand structure,
/// and when QCommand goes to the Dispatch instance it should know what device has been requested to,
/// and what type of request is it, group of rights are requested for this command, username of the requester,
/// command itself and additional payload, which varies from command to command and from device to device.
/// Remember that username should match the username got from cookies, and group of rights should match the command.
/// Actually, group name is checked on the device side, not here. Here we check if the user has the access to the group.
///
/// After calling the right function (which is defined by `qtype`) of the right device (which is defined by the GET payload),
/// with right group and username we send this QCommand to the device, where it handles group matching, matches the
/// command with its own list and calls needed function with needed payload (sometimes username can be added to that payload).
/// This function must generate actual HTML code to be inserted into the page, where all data is assembled via handlers
///


/// All requests to the device should be represented as QCommand.
///  * `qtype` - one of these: "R" (read), "W" (write), "Q" (request), "C" (confirm), "D" (dismiss) or "S" (status)
///  * `group` - requested group name
///  * `username` - username of the user, who has made the request. Should match with username, got from database by cookie
///  * `command` - name of the command, which is to be sent to the device
///  * `payload` - additional data for the command, it can be everything you want
#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct QCommand {
    pub qtype: String,
    pub group: String,
    pub username: String,
    pub command: String,
    pub payload: String,
}

/// Dispatches the QCommands between the devices. In most cases it just resolves the device by name
#[derive(Clone)]
struct Dispatch {
    file_device: FileDevice,
    root_device: RootDev,
    printer_device: PrinterDevice,
    blog_device: BlogDevice,
}

impl Dispatch {
    pub fn new(conn: Pool, redis_cred: &str, use_scraper: bool) -> Dispatch {
        let filer = FileDevice::new();
        Dispatch {
            printer_device: PrinterDevice::new(Arc::new(filer.clone())),
            file_device: filer,
            root_device: RootDev::new(&conn),
            blog_device: BlogDevice::new(redis_cred, use_scraper),
        }
    }

    pub fn resolve_by_name(&self, devname: &str) -> Result<&dyn Device, String> {
        match devname {
            "filer" => Ok(&self.file_device),
            "root" => Ok(&self.root_device),
            "printer" => Ok(&self.printer_device),
            "blogdev" => Ok(&self.blog_device),
            _ => Err("No such device".to_string())
        }
    }
}

/// Stores all needed data and dispatcher, and handles all the requests to the devices.
#[derive(Clone)]
pub struct DashBoard<'a> {
    pub mapped_devices: HashMap<String, String>,
    pub database_url: String,
    pub connections: Pool,
    pub templater: TemplateCache<'a>,
    dispatcher: Dispatch,
}


impl DashBoard<'_> {
    pub fn new<'a, 'b>(config: &'a Config) -> Result<DashBoard<'b>, String> {
        let conn: Pool = match get_connection(&config.db_config) {
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
            database_url: config.db_config.clone(),
            connections: conn.clone(),
            dispatcher: Dispatch::new(conn.clone(), &config.redis_config, config.use_scraper),
            templater: TemplateCache::new(),
        };
        ds.reload().unwrap();
        Ok(ds)
    }

    pub fn reload(&self) -> Result<(), String> {
        self.templater.load("templates")
    }

    /// Makes some validity checks and dispatches the command to the device's needed function
    pub fn dispatch(&self, username: &str, device: &str, query: QCommand) -> Result<jsVal, String> {
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

        match query.qtype.as_str() {
            "R" => fdevice.read_data(&query),
            "W" => fdevice.write_data(&query),
            "Q" => fdevice.request_query(&query),
            "C" => fdevice.confirm_query(&query),
            "D" => fdevice.dismiss_query(&query),
            "S" => fdevice.read_status(&query),
            _ => Err(format!("Unknown type of the query: {}", query.qtype))
        }
    }

    pub fn get_file_from_filer(&self, username: &str, path: &str) -> Result<Vec<u8>, String> {
        let device = &self.dispatcher.file_device;
        device.get_file(username, path)
    }
}


fn get_available_devices(pool: &Pool, mapped_devices: &HashMap<String, String>, username: &str) -> Vec<String> {
    match get_user_devices(pool, mapped_devices, username) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in get_available_devices: {:?}", e);
            Vec::new()
        }
    }
}

fn get_available_info(dasher: &DashBoard<'_>, username: &str, device: &str) -> jsVal {
    let query = QCommand {
        qtype: "S".to_string(),
        group: "rstatus".to_string(),
        username: username.to_string(),
        command: "".to_string(),
        payload: "".to_string(),
    };

    match dasher.dispatch(username, device, query) {
        Ok(d) => d,
        Err(e) => json!({"err": format!("Error on getting the available info: {}", e)})
    }
}

pub async fn dashboard_reload_templates(mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    match mdata.reload() {
        Ok(_) => Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body("Reloaded")),
        Err(err) => {
            eprintln!("Error on reload: {}", err);
            Ok(HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("Error occurred during reload. See logs for details"))
        }
    }
}

/// Handles empty request to the dashboard
pub async fn dashboard_page(id: Identity, info: web::Path<String>, mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in dashboard_page at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };
    match mdata.templater.render_template("dashboard.hbs",
                                          &json!({
                                              "devices": get_available_devices(&mdata.connections, &mdata.mapped_devices, &user),
                                              "subpage": get_available_info(&mdata, &user, info.as_str())
                                            })) {
        Ok(data) => Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body(data)),
        Err(err) => {
            eprintln!("Error in rendering dashboard: {}", err);
            Ok(HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("Error on rendering the page. Contact your administrator."))
        }
    }
}

/// Handles the QCommand requests
pub async fn dashboard_page_req(id: Identity, info: web::Path<String>,
                                form: web::Form<QCommand>, mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in dashboard_page_req at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };

    if user != form.username {
        return Ok(HttpResponse::BadRequest().body("Bad request: user names doesn't match"));
    }

    match mdata.templater.render_template("dashboard.hbs",
                                          &json!({
                                              "devices": get_available_devices(&mdata.connections, &mdata.mapped_devices, &user),
                                              "subpage": match mdata.dispatch(&user, info.as_str(), form.0) {
            Ok(d) => d,
            Err(e) => json!({"err": format!("Error on getting the available info: {}", e)})
        }
                                            })) {
        Ok(data) => Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body(data)),
        Err(err) => {
            eprintln!("Error in rendering dashboard: {}", err);
            Ok(HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("Error on rendering the page. Contact your administrator."))
        }
    }
}

/// Sends needed file to the user after security checks
pub async fn file_sender(id: Identity, info: web::Path<String>, mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    println!("File transfer");
    let cookie = match id.identity() {
        Some(data) => data,
        None => return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in file_sender at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };

    let file_data = match mdata.get_file_from_filer(&user, &info.as_str().replace("%2F", "/")) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error on getting the file: {}", e);
            return Ok(HttpResponse::BadRequest().body(format!("<html>
        <link rel=\"stylesheet\" type=\"text/css\" href=\"/static/lite.css\" media=\"screen\" />\
        <body>
            <p class=\"error\">
        Error on getting the file `{}`: {}
    </p>
        </body>
        </html>", info.as_str(), e)));
        }
    };

    println!("File size: {}", file_data.len());
    Ok(HttpResponse::Ok().set_header(http::header::CONTENT_TYPE, "multipart/form-data")
        .set_header(http::header::CONTENT_LENGTH, file_data.len())
        .set_header(http::header::CONTENT_DISPOSITION, format!("filename=\"{}\"", info.as_str().split("%2F").collect::<Vec<&str>>().pop().unwrap_or("some_file")))
        .body(file_data))
}

/// Page for uploading the file
pub async fn upload_index(id: Identity, mdata: web::Data<DashBoard<'_>>, info: web::Path<String>) -> Result<HttpResponse, Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish()),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in upload_index at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().header(http::header::LOCATION, "/login").finish());
        }
    };

    let gaccess = match has_access_to_group(&mdata.connections, &user, "filer_write") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error on dispatching (getting access to group): {}", e);
            return Ok(HttpResponse::InternalServerError().body("Error on dispatching".to_string()));
        }
    };

    if !gaccess {
        return Ok(HttpResponse::Forbidden().body("You are not allowed to upload files"));
    }
    let mut context_data: BTreeMap<String, String> = BTreeMap::new();
    context_data.insert("target".to_string(), info.to_string());
    match mdata.templater.render_template("upload.hbs", &context_data) {
        Ok(data) => Ok(HttpResponse::Ok().body(data)),
        Err(err) => {
            eprintln!("Error in rendering the page: {}", err);
            Ok(HttpResponse::InternalServerError().body("Page render error. Contact your administrator"))
        }
    }
}

/// Handles the upload requests
pub async fn uploader(id: Identity, mut multipart: Multipart, mdata: web::Data<DashBoard<'_>>, info: web::Path<String>) -> Result<HttpResponse, Error> {
    let cookie = match id.identity() {
        Some(data) => data,
        None => return Err(error::ErrorUnauthorized("Unauthorized")),
    };

    let user = match get_user_from_cookie(&mdata.connections, &cookie) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error in uploader at getting the user: {:?}", e);
            return Err(error::ErrorNotFound("Unauthorized"));
        }
    };

    let gaccess = match has_access_to_group(&mdata.connections, &user, "filer_write") {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error on uploader (getting access to group): {}", e);
            return Err(error::ErrorForbidden("You are not allowed"));
        }
    };

    if !gaccess {
        return Err(error::ErrorForbidden("You are not allowed"));
    }


    while let Some(item) = multipart.next().await {
        let mut field = match item {
            Ok(f) => f,
            Err(e) => return Err(error::ErrorBadRequest(format!("Bad item: {:?}", e)))
        };
        let file_path_string = match field.content_disposition() {
            Some(c_d) => match c_d.get_filename() {
                Some(filename) => filename.replace(' ', "_").to_string(),
                None => return Err(error::ErrorBadRequest("No filename in content-disposition"))
            },
            None => return Err(error::ErrorBadRequest("No content-disposition"))
        };
        let directory = info.to_string().replace("%2F", "/");
        let full_path = format!("{}/{}", directory, file_path_string);

        // Field in turn is stream of *Bytes* object
        while let Some(chunk) = field.next().await {
            let data = match chunk {
                Ok(d) => d,
                Err(e) => return Err(error::ErrorInternalServerError(format!("Error on getting data chunk: {:?}", e)))
            };
            // filesystem operations are blocking, we have to use threadpool
            match mdata.dispatcher.file_device.write_file(&user, &full_path, data.as_ref()) {
                Ok(_d) => (),
                Err(e) => return Err(error::ErrorInternalServerError(format!("Error on writing to buffer: {}", e)))
            };
        }

        match mdata.dispatcher.file_device.finish_file(&user, &full_path, &directory) {
            Ok(_d) => (),
            Err(e) => return Err(error::ErrorInternalServerError(format!("Error on writing to disk: {}", e)))
        };
    }

    Ok(HttpResponse::Ok().body("OK"))
}