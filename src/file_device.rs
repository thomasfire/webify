use crate::device_trait::*;
use crate::dashboard::QCommand;
use crate::io_tools::exists;

use serde_json::Value as jsVal;
use serde_json::json;
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;

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
}

impl FileDevice {
    /// Creates new instance of FileDevice
    pub fn new() -> FileDevice {
        let store = "filer".to_string();
        if !exists(&store) {
            fs::create_dir(&store).unwrap();
        }
        FileDevice { storage: store, buffered_files: Arc::new(Mutex::new(BTreeMap::new())) }
    }

    /// Returns content of the file as vector of bytes
    pub fn get_file(&self, username: &str, payload: &str) -> Result<Vec<u8>, String> {
        println!("Trying to open the file");
        let filepath = format!("{}/{}", &self.storage, username);
        if !exists(&filepath) {
            return Err("No container was found".to_string());
        }
        println!("Trying to open the file");
        let mut file = match fs::File::open(&format!("{}/{}", filepath, payload)) {
            Ok(f) => f,
            Err(e) => return Err(format!("Error on opening the file: {:?}", e))
        };

        println!("Start reading the file");
        let mut file_data: Vec<u8> = vec![];
        match file.read_to_end(&mut file_data) {
            Ok(_) => {
                let mut decoder = GzDecoder::new(&file_data[..]);
                let mut decompressed: Vec<u8> = vec![];
                match decoder.read_to_end(&mut decompressed) {
                    Ok(_) => {
                        println!("Size of decompressed: {}", decompressed.len());
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
        let filepath = format!("{}/{}/{}", &self.storage, username, payload);
        self.buffered_files.lock()
            .map(move |mut x| {
                match x.get_mut(&filepath) {
                    Some(f) => {
                        println!("Adding bytes: {}", data.len());
                        f.data.extend_from_slice(data);
                        println!("Added bytes: {}; Total: {}", data.len(), f.data.len());
                        return ();
                    }
                    None => {
                        println!("Initial Adding bytes: {}", data.len());
                        x.insert(filepath.clone(), BufferedFile { data: data.to_vec() });
                        println!("Wrote...");
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
            println!("Start writing");
            let bf_data = match x.remove(&format!("{}/{}", filepath, payload)) {
                Some(f) => f,
                None => return Err("No data to write".to_string())
            };


            let data = &bf_data.data;
            eprintln!("{}", data.len());
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
            return Err("No container was found".to_string());
        }
        if payload.contains("..") {
            return Err("Bad request".to_string());
        }
        let full_path = format!("{}/{}", filepath, payload);
        let entries = match fs::read_dir(&full_path) {
            Ok(f) => f,
            Err(e) => return Err(format!("Error on opening the directory: {:?}", e))
        };

        Ok(json!({
            "template": "file_device.hbs",
            "prepath": payload,
            "prepath_fx": payload.replace("/", "%2F"),
            "username": username,
            "entries": entries.filter(|x| x.is_ok()).map(|x| {
                match x {
                    Ok(d) => {
                        if d.path().is_file() {
                            json!({
                                "isfile": 1,
                                "filename": d.file_name().to_string_lossy()
                            })
                        } else {
                            json!({
                                "dirname": d.file_name().to_string_lossy().to_string()
                            })
                        }
                    }
                    Err(_) => json!({}),
                }
            }).collect::<jsVal>()
        }))
    }

    fn create_dir(&self, username: &str, payload: &str) -> Result<jsVal, String> {
        let filepath = format!("{}/{}", &self.storage, username);
        println!("Create {}/{}", filepath, payload);
        match fs::create_dir_all(format!("{}/{}", filepath, payload)) {
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

        if query.group != "filer_read" {
            return Err("No access to this action".to_string());
        }

        match command {
            "getlist" => self.get_list(&query.username, &query.payload),
            _ => return Err(format!("Unknown for FileDevice.read command: {}", command))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<jsVal, String> {
        if query.group != "rstatus" {
            return Err("No access to this action".to_string());
        }
        self.get_list(&query.username, &query.payload)
    }
}


impl DeviceWrite for FileDevice {
    fn write_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != "filer_write" {
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