use crate::device_trait::*;
use crate::dashboard::QCommand;
use crate::config::Config;
use crate::devices::{Devices, Groups, DEV_GROUPS};

use serde_json::{Value as jsVal, json, from_str as js_from_str};
use reqwest;
use log::{debug};


#[derive(Clone)]
pub struct EcgDevice {
    server_addr: String,
}

impl EcgDevice {
    pub fn new(config: &Config) -> Self {
        EcgDevice { server_addr: config.ecg_server.clone() }
    }
}

impl DeviceRead for EcgDevice {
    fn read_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != DEV_GROUPS[Devices::ECG as usize][Groups::Read as usize].unwrap() {
            return Err("No access to this action".to_string());
        }
        match command {
            "read" => (),
            _ => return Err(format!("Unknown command"))
        };

        let lnurl = format!("{}/{}", self.server_addr.strip_suffix("/").unwrap_or(""), query.payload);
        let body = reqwest::blocking::get(lnurl)
            .map_err(|err| { format!("Reqwest to ECG server failed at article: {:?}", err) })?.text()
            .map_err(|err| { format!("Reqwest to ECG server failed at article: {:?}", err) })?;
        let js_body: jsVal = js_from_str(&body).unwrap_or(json!([]));

        if !js_body.is_object() {
            return Err("Responce from ECG server is not a valid JSON object".to_string());
        }

        Ok(json!({
            "template": "ecg_webapp.hbs",
            "username": query.username.clone(),
            "server": self.server_addr.clone(),
            "file": query.payload.clone(),
            "data": js_body,
        }))
    }

    fn read_status(&self, query: &QCommand) -> Result<jsVal, String> {
        if query.group != DEV_GROUPS[Devices::Zero as usize][Groups::RStatus as usize].unwrap() {
            return Err("No access to this action".to_string());
        }
        let lnurl = format!("{}/list", self.server_addr.strip_suffix("/").unwrap_or(""));
        let body = reqwest::blocking::get(lnurl)
            .map_err(|err| { format!("Reqwest to ECG server failed at request: {:?}", err) })?.text()
            .map_err(|err| { format!("Reqwest to ECG server failed at request: {:?}", err) })?;
        let js_body: jsVal =  js_from_str(&body).unwrap_or(json!([]));

        debug!("got: {}", body);
        if !js_body.is_array() {
            return Err("Response from ECG server is not a valid JSON array".to_string());
        }

        debug!("JSON: {}", js_body.to_string());
        Ok(json!({
            "template": "ecg_status.hbs",
            "username": query.username,
            "server": self.server_addr.clone(),
            "entries": js_body,
        }))
    }
}


impl DeviceWrite for EcgDevice {
    fn write_data(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceRequest for EcgDevice {
    fn request_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceConfirm for EcgDevice {
    fn confirm_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}
