use crate::device_trait::*;

use std::io;
use std::io::prelude::*;

use diesel::{r2d2, SqliteConnection};
use diesel::r2d2::ConnectionManager;
use crate::dashboard::QCommand;
use std::fs;
use crate::io_tools::exists;
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;
use std::io::BufWriter;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::borrow::BorrowMut;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

struct BufferedFile {
    pub path: String,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct FileDevice {
    db_conn: Pool,
    storage: String,
    buffered_files: Arc<Mutex<BTreeMap<String, BufferedFile>>>,
}

impl FileDevice {
    pub fn new(conn: &Pool) -> FileDevice {
        let store = "filer".to_string();
        if !exists(&store) {
            fs::create_dir(&store).unwrap();
        }
        FileDevice { db_conn: conn.clone(), storage: store, buffered_files: Arc::new(Mutex::new(BTreeMap::new())) }
    }

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

    pub fn write_file(&mut self, username: &str, payload: &str, data: &[u8]) -> Result<(), String> {
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
                        x.insert(filepath.clone(), BufferedFile { path: filepath.clone(), data: data.to_vec() });
                        println!("Wrote...");
                        return ();
                    }
                };
            }).map_err(|x| {
            return format!("Internal error");
        })
    }

    pub fn finish_file(&mut self, username: &str, payload: &str, directory: &str) -> Result<(), String> {
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

    fn get_list(&self, username: &str, payload: &str) -> Result<String, String> {
        let filepath = format!("{}/{}", &self.storage, username);
        if !exists(&filepath) {
            return Err("No container was found".to_string());
        }
        let full_path = format!("{}/{}", filepath, payload);
        let entries = match fs::read_dir(&full_path) {
            Ok(f) => f,
            Err(e) => return Err(format!("Error on opening the directory: {:?}", e))
        };
        let list: Vec<String> = entries.filter(|x| x.is_ok())
            .map(|x| {
                match x {
                    Ok(d) => {
                        if d.path().is_file() {
                            format!("<div class=\"item\"><a href=\"../download/{}%2F{}\">{}</div>", payload.replace("/", "%2F"), d.file_name().to_string_lossy(), d.file_name().to_string_lossy())
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

        Ok(list.join("<br>"))
    }

    fn create_dir(&self, username: &str, payload: &str) -> Result<String, String> {
        let filepath = format!("{}/{}.tar", &self.storage, username);

        match fs::create_dir_all(format!("{}/{}", filepath, payload)) {
            Ok(_) => return Ok("OK".to_string()),
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

    fn read_status(&self) -> Result<String, String> {
        Ok(format!("FileDevice is ready"))
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
    fn request_query(&self, query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceConfirm for FileDevice {
    fn confirm_query(&self, query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }
}