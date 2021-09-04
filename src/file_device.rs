use crate::device_trait::*;

use std::io::prelude::*;

use crate::dashboard::QCommand;
use std::fs;
use crate::io_tools::exists;
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

struct BufferedFile {
    //pub path: String,
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
    fn get_list(&self, username: &str, payload: &str) -> Result<String, String> {
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
        let mut list: Vec<String> = entries.filter(|x| x.is_ok())
            .map(|x| {
                match x {
                    Ok(d) => {
                        if d.path().is_file() {
                            format!("<div class=\"item\"><a href=\"../download/{}%2F{}\">{}</a></div>", payload.replace("/", "%2F"), d.file_name().to_string_lossy(), d.file_name().to_string_lossy())
                        } else {
                            let name = d.file_name().to_string_lossy().to_string();
                            format!(r#"<div class="linked_form">
                                        <form action="/dashboard/filer"  method="post" id="dir_sender{}">
                                            <div class="command_f">
                                              <input type="hidden" name="qtype" value="R" class="qtype">
                                              <input type="hidden" name="group" value="filer_read" class="group">
                                              <input type="hidden" name="username" value="{}" class="username">
                                              <input type="hidden" name="command" value="getlist" class="command">
                                              <input type="hidden" name="payload" value="{}{}" class="payload">
                                            </div>
                                              <a href=" #" onclick="document.getElementById('dir_sender{}').submit();">{}</a>
                                        </form>
                            </div>"#, name, username, if payload.len() > 0 { payload.to_string() + "/" } else { "".to_string() }, name, name, name)
                        }
                    }
                    Err(_) => format!(""),
                }
            }).collect();

        list.push(format!(r#"<br><br><div class="createnew_form">
                                        <form action="/dashboard/filer" method="post" id="create_new">
                                            <div class="command_f">
                                              <input type="hidden" name="qtype" value="W" class="qtype">
                                              <input type="hidden" name="group" value="filer_write" class="group">
                                              <input type="hidden" name="username" value="{}" class="username">
                                              <input type="hidden" name="command" value="createdir" class="command">
                                              <input type="text" name="payload" value="{}" class="payload">
                                            </div>
                                            <div class="createnew_link">
                                              <a href=" #" onclick="document.getElementById('create_new').submit();">Create new dir</a>
                                            </div>
                                        </form>
                            </div>"#, username, if payload.len() > 0 { payload.to_string() + "/" } else { "".to_string() }));

        list.push(format!(r#"<br>
        <div class="uploader">
                Upload a file<br>
                <form target="../../upload/{}" action="../../upload/{}" method="post" enctype="multipart/form-data">
                    <input type="file" name="file"/><br>
                    <input type="submit" value="Senden">
                </form>
            </div>
        "#, payload.replace("/", "%2F"), payload.replace("/", "%2F")));

        Ok(list.join("<br>"))
    }

    fn create_dir(&self, username: &str, payload: &str) -> Result<String, String> {
        let filepath = format!("{}/{}", &self.storage, username);
        println!("Create {}/{}", filepath, payload);
        match fs::create_dir_all(format!("{}/{}", filepath, payload)) {
            Ok(_) => return Ok(match self.get_list(username, payload) {
                Ok(r) => r,
                Err(e) => format!("Error on getting list after created the dir: {}", e)
            }),
            Err(e) => return Err(format!("Error on making the directories: {:?}", e))
        };
    }
}


impl DeviceRead for FileDevice {
    fn read_data(&self, query: &QCommand) -> Result<String, String> {
        let command = query.command.as_str();

        if query.group != "filer_read" {
            return Err("No access to this action".to_string());
        }

        match command {
            "getlist" => self.get_list(&query.username, &query.payload),
            _ => return Err(format!("Unknown for FileDevice.read command: {}", command))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<String, String> {
        if query.group != "rstatus" {
            return Err("No access to this action".to_string());
        }
        self.get_list(&query.username, &query.payload)
    }
}


impl DeviceWrite for FileDevice {
    fn write_data(&self, query: &QCommand) -> Result<String, String> {
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
    fn request_query(&self, _query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceConfirm for FileDevice {
    fn confirm_query(&self, _query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, _query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }
}