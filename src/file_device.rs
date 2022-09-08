use crate::device_trait::*;
use crate::dashboard::QCommand;
use crate::io_tools::exists;
use crate::devices::{Devices, Groups, DEV_GROUPS};
use crate::database::Database;

use serde_json::Value as jsVal;
use serde_json::json;
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;
use log::{debug, info};
use urlencoding;

use std::fs;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::io::prelude::*;

struct BufferedFile {
    pub data: Vec<u8>,
}

/// Contains path to the storage, and buffered files. And yes, files are stored in the RAM before
/// actual writing.
#[derive(Clone)]
pub struct FileDevice {
    storage: String,
    buffered_files: Arc<Mutex<BTreeMap<String, BufferedFile>>>,
    database: Database,
}

impl FileDevice {
    /// Creates new instance of FileDevice
    pub fn new(database: &Database) -> FileDevice {
        let store = "filer".to_string();
        if !exists(&store) {
            fs::create_dir(&store).unwrap();
        }
        FileDevice {
            storage: store,
            buffered_files: Arc::new(Mutex::new(BTreeMap::new())),
            database: database.clone(),
        }
    }

    /// Returns content of the file as vector of bytes
    pub fn get_file(&self, username: &str, payload: &str) -> Result<Vec<u8>, String> {
        debug!("Trying to open the file");
        if payload.contains("..") {
            return Err("Forbidden path".to_string());
        }
        let filepath = format!("{}/{}", &self.storage, username);
        if !exists(&filepath) {
            return Err("No container was found".to_string());
        }
        let mut file = match fs::File::open(
            &format!("{}/{}", filepath, urlencoding::decode(payload).map_err(|_| { format!("Couldn't decode payload: `{}`", payload) })?)) {
            Ok(f) => f,
            Err(e) => return Err(format!("Error on opening the file: {:?}", e))
        };

        debug!("Start reading the file");
        let mut file_data: Vec<u8> = vec![];
        match file.read_to_end(&mut file_data) {
            Ok(_) => {
                let mut decoder = GzDecoder::new(&file_data[..]);
                let mut decompressed: Vec<u8> = vec![];
                match decoder.read_to_end(&mut decompressed) {
                    Ok(_) => {
                        info!("Size of decompressed: {}", decompressed.len());
                        return Ok(decompressed);
                    }
                    Err(e) => return Err(format!("Error on decompressing the file: {}", e)),
                }
            }
            Err(e) => return Err(format!("Error on reading the file: {:?}", e))
        };
    }

    /// Writes content to the the RAM
    pub fn write_file(&self, username: &str, payload: &str, data: &[u8]) -> Result<(), String> {
        if payload.contains("..") {
            return Err("Wrong symbols were supplied".to_string());
        }
        let filepath = format!("{}/{}/{}",
                               &self.storage,
                               username,
                               urlencoding::decode(payload).map_err(|_| { format!("Couldn't decode payload: `{}`", payload) })?);
        self.buffered_files.lock()
            .map(move |mut x| {
                match x.get_mut(&filepath) {
                    Some(f) => {
                        debug!("Adding bytes: {}", data.len());
                        f.data.extend_from_slice(data);
                        info!("Added bytes: {}; Total: {}", data.len(), f.data.len());
                        return ();
                    }
                    None => {
                        debug!("Initial Adding bytes: {}", data.len());
                        x.insert(filepath.clone(), BufferedFile { data: data.to_vec() });
                        debug!("Wrote...");
                        return ();
                    }
                };
            }).map_err(|_x| {
            return format!("Internal error");
        })
    }

    /// Writes file from buffer to the disk after being compressed
    pub fn finish_file(&self, username: &str, payload: &str, directory: &str) -> Result<(), String> {
        let filepath = format!("{}/{}", &self.storage, username);

        let mut file = if exists(&format!("{}/{}", filepath, directory)) {
            match fs::File::create(format!("{}/{}", filepath, payload)) {
                Ok(f) => f,
                Err(e) => return Err(format!("Error on creating the file: {:?}", e))
            }
        } else {
            match fs::create_dir_all(format!("{}/{}", filepath, directory)) {
                Ok(_) => (),
                Err(e) => return Err(format!("Error on making the directories: {:?}", e))
            };
            match fs::File::create(format!("{}/{}", filepath, payload)) {
                Ok(f) => f,
                Err(e) => return Err(format!("Error on creating the file: {:?}", e))
            }
        };

        let res: Result<Result<(), String>, String> = self.buffered_files.lock().map(move |mut x| {
            debug!("Start writing");
            let bf_data = match x.remove(&format!("{}/{}", filepath, payload)) {
                Some(f) => f,
                None => return Err("No data to write".to_string())
            };


            let data = &bf_data.data;
            info!("Total file len: {}", data.len());
            let mut file_compressed: Vec<u8> = vec![];
            let mut encoder = GzEncoder::new(&data[..], Compression::best());
            match encoder.read_to_end(&mut file_compressed) {
                Ok(_) => (),
                Err(e) => return Err(format!("Error on compressing the file: {:?}", e))
            };

            match file.write_all(&file_compressed) {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("Error on writing the file: {:?}", e))
            }
        }).map_err(|x| {
            return format!("Error on finishing the file: {}", x);
        });

        match res {
            Ok(another) => another,
            Err(e) => Err(e)
        }
    }

    /// Returns the list of files
    fn get_list(&self, username: &str, payload: &str) -> Result<jsVal, String> {
        let filepath = format!("{}/{}", &self.storage, username);
        if !exists(&filepath) {
            if !self.database.has_access_to_group(username, DEV_GROUPS[Devices::Filer as usize][Groups::Write as usize].unwrap())
                .unwrap_or(false) {
                return Err("No storage for current user and no access for creating it".to_string());
            }
            match fs::create_dir(&filepath) {
                Ok(_) => (),
                Err(err) => return Err(format!("No container was found and couldn't create new: {:?}", err))
            };
        }
        let paths = urlencoding::decode(payload).map_err(|_| { format!("Couldn't decode payload: `{}`", payload) })?.to_string();
        if paths.contains("..") {
            return Err("Bad request".to_string());
        }
        let full_path = format!("{}/{}", filepath, paths);
        let mut entries: Vec<jsVal> = match fs::read_dir(&full_path) {
            Ok(f) => f.filter(|x| x.is_ok()).map(|x| {
                match x {
                    Ok(d) => {
                        if d.path().is_file() {
                            json!({
                                "isfile": 1,
                                "filename": d.file_name().to_string_lossy()
                            })
                        } else {
                            let fname = d.file_name().to_string_lossy().to_string();
                            json!({
                                "full_path": urlencoding::encode(&format!("{}/{}", paths, fname)),
                                "display": fname,
                            })
                        }
                    }
                    Err(_) => json!({}),
                }
            }).collect::<Vec<jsVal>>(),
            Err(e) => return Err(format!("Error on opening the directory: {:?}", e))
        };
        entries.insert(0, json!({
                                "full_path": &paths,
                                "display": "."
                            }));
        let last = paths.rfind("/");
        if last.is_some() {
            entries.insert(1, json!({
                                "full_path": &paths[..last.unwrap()],
                                "display": ".."
                            }));
        }


        Ok(json!({
            "template": "file_device.hbs",
            "prepath": paths,
            "prepath_fx": payload,
            "username": username,
            "entries": entries
        }))
    }

    fn create_dir(&self, username: &str, payload: &str) -> Result<jsVal, String> {
        let filepath = format!("{}/{}", &self.storage, username);
        let paths = urlencoding::decode(payload).map_err(|_| { format!("Couldn't decode payload: `{}`", payload) })?.to_string();
        debug!("Create {}/{}", filepath, paths);
        match fs::create_dir_all(format!("{}/{}", filepath, paths)) {
            Ok(_) => return Ok(match self.get_list(username, payload) {
                Ok(r) => r,
                Err(e) => return Err(format!("Error on getting list after created the dir: {}", e))
            }),
            Err(e) => return Err(format!("Error on making the directories: {:?}", e))
        };
    }
}


impl DeviceRead for FileDevice {
    fn read_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != DEV_GROUPS[Devices::Filer as usize][Groups::Read as usize].unwrap() {
            return Err("No access to this action".to_string());
        }

        match command {
            "getlist" => self.get_list(&query.username, &query.payload),
            _ => return Err(format!("Unknown for FileDevice.read command: {}", command))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<jsVal, String> {
        if query.group != DEV_GROUPS[Devices::Zero as usize][Groups::RStatus as usize].unwrap() {
            return Err("No access to this action".to_string());
        }
        self.get_list(&query.username, &query.payload)
    }
}


impl DeviceWrite for FileDevice {
    fn write_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != DEV_GROUPS[Devices::Filer as usize][Groups::Write as usize].unwrap() {
            return Err("No access to this action".to_string());
        }

        match command {
            "createdir" => self.create_dir(&query.username, &query.payload),
            _ => return Err(format!("Unknown for FileDevice.read command: {}", command))
        }
    }
}


impl DeviceRequest for FileDevice {
    fn request_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceConfirm for FileDevice {
    fn confirm_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}