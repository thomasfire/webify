use crate::device_trait::*;

use std::io;
use std::io::prelude::*;

use diesel::{r2d2, SqliteConnection};
use diesel::r2d2::ConnectionManager;
use crate::dashboard::QCommand;
use std::fs;
use crate::io_tools::exists;
use tar;
use flate2::read::{GzDecoder, GzEncoder};
use flate2::Compression;
use std::io::BufWriter;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone)]
pub struct FileDevice {
    db_conn: Pool,
    storage: String,
}

impl FileDevice {
    pub fn new(conn: &Pool) -> FileDevice {
        let store = "filer".to_string();
        if !exists(&store) {
            fs::create_dir(&store).unwrap();
        }
        FileDevice { db_conn: conn.clone(), storage: store }
    }

    pub fn get_file(&self, username: &str, payload: &str) -> Result<Vec<u8>, String> {
        let filepath = format!("{}/{}.tar", &self.storage, username);
        if !exists(&filepath) {
            return Err("No container was found".to_string());
        }

        let file = match fs::File::open(&filepath) {
            Ok(f) => f,
            Err(e) => return Err(format!("Error on opening the file: {:?}", e))
        };
        let mut ar = tar::Archive::new(file);
        let entries = match ar.entries() {
            Ok(d) => d,
            Err(e) => return Err(format!("Error on reading the file: {:?}", e))
        };

        let mut file_data: Vec<u8> = vec![];

        for x in entries {
            let mut f_e: tar::Entry<fs::File> = match x {
                Ok(d) => d,
                Err(e) => return Err(format!("Error on reading the container: {:?}", e))
            };
            let name = match f_e.path() {
                Ok(d) => d.as_os_str().to_str().unwrap_or("").to_string(),
                Err(e) => return Err(format!("Error on reading the paths: {:?}", e))
            };
            let is_correct = name.starts_with(payload);
            if is_correct {
                match f_e.read_to_end(&mut file_data) {
                    Ok(_) => {
                        let mut decoder = GzDecoder::new(&file_data[..]);
                        let mut decompressed: Vec<u8> = vec![];
                        match decoder.read_to_end(&mut decompressed) {
                            Ok(_) => return Ok(decompressed),
                            Err(e) => return Err(format!("Error on decompressing the file: {}", e)),
                        }
                    }
                    Err(e) => return Err(format!("Error on reading the file: {:?}", e))
                }
            }
        }
        Err("Couldn't find a file".to_string())
    }


    pub fn write_file(&self, username: &str, payload: &str, data: &Vec<u8>) -> Result<(), String> {
        let filepath = format!("{}/{}.tar", &self.storage, username);

        let file = if exists(&filepath) {
            match fs::File::open(&filepath) {
                Ok(f) => f,
                Err(e) => return Err(format!("Error on opening the file: {:?}", e))
            }
        } else {
            match fs::File::create(&filepath) {
                Ok(f) => f,
                Err(e) => return Err(format!("Error on creating the file: {:?}", e))
            }
        };
        let bfile = BufWriter::new(file);
        eprintln!("{}", data.len());
        let mut ar = tar::Builder::new(bfile);
        let mut file_compressed: Vec<u8> = vec![];
        let mut encoder = GzEncoder::new(&data[..], Compression::best());
        match encoder.read_to_end(&mut file_compressed) {
            Ok(_) => (),
            Err(e) => return Err(format!("Error on compressing the file: {:?}", e))
        };

        let mut head = tar::Header::new_gnu();
        match head.set_path(payload) {
            Ok(_) => (),
            Err(e) => return Err(format!("Error on setting the filepath: {:?}", e))
        };
        head.set_size(file_compressed.len() as u64);
        head.set_cksum();
        //head.set_mode();

        match ar.append_data(&mut head, payload,  &file_compressed[..]) {
            Ok(_) => (),
            Err(e) => return Err(format!("Error on writing to the archive: {:?}", e))
        };

        match ar.into_inner() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Error on finishing the archive: {:?}", e))
        }
    }

    fn get_list(&self, username: &str, payload: &str) -> Result<String, String> {
        let filepath = format!("{}/{}.tar", &self.storage, username);
        if !exists(&filepath) {
            return Err("No container was found".to_string());
        }
        let file = match fs::File::open(&filepath) {
            Ok(f) => f,
            Err(e) => return Err(format!("Error on opening the file: {:?}", e))
        };
        let mut ar = tar::Archive::new(file);

        let entries = match ar.entries() {
            Ok(d) => d,
            Err(e) => return Err(format!("Error on reading the file: {:?}", e))
        };

        let mut list: Vec<String> = vec![];

        for x in entries {
            let f_e: tar::Entry<fs::File> = match x {
                Ok(d) => d,
                Err(e) => return Err(format!("Error on reading the container: {:?}", e))
            };
            let name = match f_e.path() {
                Ok(d) => d.as_os_str().to_str().unwrap_or("").to_string(),
                Err(e) => return Err(format!("Error on reading the paths: {:?}", e))
            };
            let is_correct = name.starts_with(payload);
            if is_correct {
                list.push(format!("<div class=\"item\"><a href=\"../download/{}\">{}</div>", name, name.trim_start_matches(payload)));
            }
        }

        Ok(list.join("<br>"))
    }

    fn create_dir(&self, username: &str, payload: &str) -> Result<String, String> {
        let filepath = format!("{}/{}.tar", &self.storage, username);

        let file = if exists(&filepath) {
            match fs::File::open(&filepath) {
                Ok(f) => f,
                Err(e) => return Err(format!("Error on opening the file: {:?}", e))
            }
        } else {
            match fs::File::create(&filepath) {
                Ok(f) => f,
                Err(e) => return Err(format!("Error on creating the file: {:?}", e))
            }
        };

        let mut ar = tar::Builder::new(file);

        match fs::create_dir_all(format!("temp/{}", payload)) {
            Ok(_) => (),
            Err(e) => return Err(format!("Error on creating dir: {:?}", e))
        };
        match ar.append_dir_all(payload,  &format!("temp/{}", payload)) {
            Ok(_) => (),
            Err(e) => return Err(format!("Error on writing to the archive: {:?}", e))
        };

        match fs::remove_dir_all("temp") {
            Ok(_) => (),
            Err(e) => return Err(format!("Error on removing the dir: {:?}", e))
        };
        match ar.into_inner() {
            Ok(_) => Ok("".to_string()),
            Err(e) => Err(format!("Error on finishing the archive: {:?}", e))
        }
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