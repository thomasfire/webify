extern crate actix_web;
extern crate actix_form_data;

use crate::database::{Database};
use crate::root_device::RootDev;
use crate::device_trait::*;
use crate::file_device::FileDevice;
use crate::printer_device::PrinterDevice;
use crate::blog_device::BlogDevice;
use crate::config::Config;
use crate::template_cache::TemplateCache;
use crate::models::RejectReason;
use crate::stat_device::StatDevice;
use crate::ecg_device::EcgDevice;
use crate::devices;
use crate::server::AUTH_COOKIE;

use actix_web::{Error, HttpResponse, web, error, http, HttpRequest};
use futures::StreamExt;
use serde_json::Value as jsVal;
use serde_json::json;
use actix_multipart::Multipart;
use log::{debug, error, warn, trace};
use urlencoding;

use std::sync::Arc;
use std::collections::BTreeMap;
use std::convert::TryInto;

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
    stat_device: StatDevice,
    ecg_device: EcgDevice,
}

impl Dispatch {
    pub fn new(database: &Database, config: &Config) -> Dispatch {
        let filer = FileDevice::new(database);
        Dispatch {
            printer_device: PrinterDevice::new(Arc::new(filer.clone())),
            file_device: filer,
            root_device: RootDev::new(database),
            blog_device: BlogDevice::new(&config.redis_config, database, config.use_scraper),
            stat_device: StatDevice::new(database, config),
            ecg_device: EcgDevice::new(config),
        }
    }

    pub fn resolve_by_name(&self, devname: &str) -> Result<&dyn Device, String> {
        let index = match devices::DEV_NAMES.iter().position(|x| x == &devname) {
            Some(v) => v,
            None => return Err("No such device".to_string())
        };

        match index.try_into() {
            Ok(devices::Devices::Filer) => Ok(&self.file_device),
            Ok(devices::Devices::Printer) => Ok(&self.printer_device),
            Ok(devices::Devices::Root) => Ok(&self.root_device),
            Ok(devices::Devices::Blog) => Ok(&self.blog_device),
            Ok(devices::Devices::Stat) => Ok(&self.stat_device),
            Ok(devices::Devices::ECG) => Ok(&self.ecg_device),
            _ => Err("No such device".to_string())
        }
    }
}

/// Stores all needed data and dispatcher, and handles all the requests to the devices.
#[derive(Clone)]
pub struct DashBoard<'a> {
    pub database: Database,
    pub templater: TemplateCache<'a>,
    dispatcher: Dispatch,
}


impl DashBoard<'_> {
    pub fn new<'a, 'b>(config: &'a Config) -> Result<DashBoard<'b>, String> {
        let database = Database::new(config.db_config.as_str(), config.redis_cache.as_str()).unwrap();
        let ds: DashBoard = DashBoard {
            dispatcher: Dispatch::new(&database, &config),
            database: database,
            templater: TemplateCache::new(),
        };
        ds.reload().unwrap();
        Ok(ds)
    }

    pub fn reload(&self) -> Result<(), String> {
        self.database.devices_reload()?;
        self.templater.load("templates")
    }

    /// Makes some validity checks and dispatches the command to the device's needed function
    pub fn dispatch(&self, username: &str, device: &str, query: QCommand) -> Result<jsVal, String> {
        if username != query.username {
            return Err(format!("Wrong command credentials"));
        }

        let daccess = match self.database.has_access_to_device(username, device) {
            Ok(d) => d,
            Err(e) => {
                error!("Error on dispatching (getting access to dev): {}", e);
                return Err("Error on dispatching".to_string());
            }
        };

        let gaccess = match self.database.has_access_to_group(username, &query.group) {
            Ok(d) => d,
            Err(e) => {
                error!("Error on dispatching (getting access to group): {}", e);
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

fn get_available_info(dasher: &DashBoard<'_>, username: &str, device: &str) -> jsVal {
    let query = QCommand {
        qtype: "S".to_string(),
        group: "rstatus".to_string(),
        username: username.to_string(),
        command: "rstatus".to_string(),
        payload: "".to_string(),
    };

    let (j_val, reject) = match dasher.dispatch(username, device, query.clone()) {
        Ok(d) => (d, RejectReason::Ok as i32),
        Err(e) => (json!({"err": format!("Error on getting the available info: {}", e)}), RejectReason::NoAuth as i32)
    };
    match dasher.database.insert_history(username, device, &query.command, &query.qtype, reject) {
        Ok(_) => (),
        Err(err) => error!("Error on inserting to the history in get_available_info: {}", err),
    };
    j_val
}

#[cfg(debug_assertions)]
pub async fn dashboard_reload_templates(mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    match mdata.reload() {
        Ok(_) => Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body("Reloaded")),
        Err(err) => {
            error!("Error on reload: {}", err);
            Ok(HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("Error occurred during reload. See logs for details"))
        }
    }
}

#[cfg(not(debug_assertions))]
pub async fn dashboard_reload_templates(_mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    Ok(HttpResponse::BadRequest()
        .content_type("text/html; charset=utf-8")
        .body("Not such feature"))
}

/// Handles empty request to the dashboard
pub async fn dashboard_page(req: HttpRequest, info: web::Path<String>, mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    let cookie = match req.cookie(AUTH_COOKIE) {
        Some(data) => data.value().to_string(),
        None => return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish()),
    };

    let user = match mdata.database.get_user_from_cookie(&cookie) {
        Ok(data) => data,
        Err(e) => {
            warn!("Error in dashboard_page at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish());
        }
    };
    let inner_info = get_available_info(&mdata, &user, info.as_str());
    let inner_template = mdata.templater.render_template(&inner_info.get("template").unwrap_or(&json!("")).as_str().unwrap_or(""), &inner_info);
    match mdata.templater.render_template("dashboard.hbs",
                                          &json!({
                                              "devices": mdata.database.get_user_devices(&user).unwrap_or(vec![]),
                                              "err": match inner_info.get("err") {
                                                  Some(v) => v.as_str().unwrap_or(""),
                                                  None => match &inner_template{Ok(_) => "", Err(err) => err}
                                              },
                                              "subpage": match &inner_template{Ok(data) => data, Err(_) => ""}
                                            })) {
        Ok(data) => Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body(data)),
        Err(err) => {
            error!("Error in rendering dashboard: {}", err);
            Ok(HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("Error on rendering the page. Contact your administrator."))
        }
    }
}

/// Handles the QCommand requests
pub async fn dashboard_page_req(req: HttpRequest, info: web::Path<String>,
                                form: web::Form<QCommand>, mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    let cookie = match req.cookie(AUTH_COOKIE) {
        Some(data) => data.value().to_string(),
        None => return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish()),
    };

    let user = match mdata.database.get_user_from_cookie(&cookie) {
        Ok(data) => data,
        Err(e) => {
            error!("Error in dashboard_page_req at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish());
        }
    };

    if user != form.username {
        return Ok(HttpResponse::BadRequest().body("Bad request: user names doesn't match"));
    }

    let (inner_info, reject) = match mdata.dispatch(&user, info.as_str(), form.0.clone()) {
        Ok(d) => (d, RejectReason::Ok as i32),
        Err(e) => (json!({"err": format!("Error on getting the available info: {}", e)}), RejectReason::NoAuth as i32)
    };
    match mdata.database.insert_history(&user, info.as_str(), &form.0.command, &form.0.qtype, reject) {
        Ok(_) => (),
        Err(err) => error!("Error on inserting to the history: {}", err),
    };
    let inner_template = mdata.templater.render_template(&inner_info.get("template").unwrap_or(&json!("")).as_str().unwrap_or(""), &inner_info);
    match mdata.templater.render_template("dashboard.hbs",
                                          &json!({
                                              "devices": mdata.database.get_user_devices(&user).unwrap_or(vec![]),
                                              "err": match inner_info.get("err") {
                                                  Some(v) => v.as_str().unwrap_or(""),
                                                  None => match &inner_template{Ok(_) => "", Err(err) => err}
                                              },
                                              "subpage": match &inner_template{Ok(data) => data, Err(_) => ""}
                                            })) {
        Ok(data) => Ok(HttpResponse::Ok().content_type("text/html; charset=utf-8").body(data)),
        Err(err) => {
            error!("Error in rendering dashboard: {}", err);
            Ok(HttpResponse::InternalServerError()
                .content_type("text/html; charset=utf-8")
                .body("Error on rendering the page. Contact your administrator."))
        }
    }
}

/// Sends needed file to the user after security checks
pub async fn file_sender(req: HttpRequest, info: web::Path<String>, mdata: web::Data<DashBoard<'_>>) -> Result<HttpResponse, Error> {
    trace!("File transfer");
    let cookie = match req.cookie(AUTH_COOKIE) {
        Some(data) => data.value().to_string(),
        None => return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish()),
    };

    let user = match mdata.database.get_user_from_cookie(&cookie) {
        Ok(data) => data,
        Err(e) => {
            error!("Error in file_sender at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish());
        }
    };

    let file_data = match mdata.get_file_from_filer(&user, &info.as_str()) {
        Ok(d) => d,
        Err(e) => {
            error!("Error on getting the file: {}", e);
            match mdata.templater.render_template("sender_error.hbs", &json!({
                "filename": info.as_str(),
                "error_msg": e
            })) {
                Ok(htmld) => return Ok(HttpResponse::BadRequest().body(htmld)),
                Err(err) => {
                    error!("Error on rendering template: {}", err);
                    return Ok(HttpResponse::InternalServerError().body("Internal error"));
                }
            }
        }
    };
    let paths = urlencoding::decode(info.as_str())
        .map_err(|_| { error::ErrorBadRequest(format!("Cannot decode directory: `{}`", &info.to_string())) })?
        .to_string();
    debug!("File size: {}", file_data.len());
    Ok(HttpResponse::Ok().insert_header((http::header::CONTENT_TYPE, "multipart/form-data"))
        .insert_header((http::header::CONTENT_LENGTH, file_data.len()))
        .insert_header((http::header::CONTENT_DISPOSITION, format!("filename=\"{}\"", paths.split("/").collect::<Vec<&str>>().pop().unwrap_or("some_file"))))
        .body(file_data))
}

/// Page for uploading the file
pub async fn upload_index(req: HttpRequest, mdata: web::Data<DashBoard<'_>>, info: web::Path<String>) -> Result<HttpResponse, Error> {
    let cookie = match req.cookie(AUTH_COOKIE) {
        Some(data) => data.value().to_string(),
        None => return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish()),
    };

    let user = match mdata.database.get_user_from_cookie(&cookie) {
        Ok(data) => data.to_string(),
        Err(e) => {
            error!("Error in upload_index at getting the user: {:?}", e);
            return Ok(HttpResponse::TemporaryRedirect().append_header((http::header::LOCATION, "/login")).finish());
        }
    };

    let gaccess = match mdata.database.has_access_to_group(&user, "filer_write") {
        Ok(d) => d,
        Err(e) => {
            error!("Error on dispatching (getting access to group): {}", e);
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
            error!("Error in rendering the page: {}", err);
            Ok(HttpResponse::InternalServerError().body("Page render error. Contact your administrator"))
        }
    }
}

/// Handles the upload requests
pub async fn uploader(req: HttpRequest, mut multipart: Multipart, mdata: web::Data<DashBoard<'_>>, info: web::Path<String>) -> Result<HttpResponse, Error> {
    let cookie = match req.cookie(AUTH_COOKIE) {
        Some(data) => data.value().to_string(),
        None => return Err(error::ErrorUnauthorized("Unauthorized")),
    };

    let user = match mdata.database.get_user_from_cookie(&cookie) {
        Ok(data) => data,
        Err(e) => {
            error!("Error in uploader at getting the user: {:?}", e);
            return Err(error::ErrorNotFound("Unauthorized"));
        }
    };

    let gaccess = match mdata.database.has_access_to_group(&user, "filer_write") {
        Ok(d) => d,
        Err(e) => {
            error!("Error on uploader (getting access to group): {}", e);
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
        let file_path_string = match field.content_disposition().get_filename() {
            Some(filename) => urlencoding::decode(filename)
                .map_err(|_| { error::ErrorBadRequest(format!("Cannot decode filename: `{}`", filename)) })?
                .to_string(),
            None => return Err(error::ErrorBadRequest("No filename in content-disposition"))
        };
        let directory = urlencoding::decode(&info.to_string())
            .map_err(|_| { error::ErrorBadRequest(format!("Cannot decode directory: `{}`", &info.to_string())) })?
            .to_string();
        let full_path = format!("{}/{}", directory, file_path_string);

        if full_path.contains("..") {
            return Err(error::ErrorForbidden("Forbidden path"));
        }

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