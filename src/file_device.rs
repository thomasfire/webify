use crate::device_trait::*;

use diesel::{r2d2, SqliteConnection};
use diesel::r2d2::ConnectionManager;
use crate::dashboard::QCommand;
use std::fs;
use crate::io_tools::exists;
use tar;

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
                list.push(format!("<div class=\"item\">{}</div>", name.trim_start_matches(payload)));
            }
        }

        Ok(list.join("<br>"))
    }
}


impl DeviceRead for FileDevice {
    fn read_data(&self, query: &QCommand) -> Result<String, String> {
        let command = query.command.as_str();

        if query.group != "file_read" {
            return Err("No access to this action".to_string());
        }

        match command {
            "getlist" => unimplemented!(),
            "getfile" => unimplemented!(),
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

        if query.group != "file_write" {
            return Err("No access to this action".to_string());
        }

        match command {
            "createdir" => unimplemented!(),
            "writefile" => unimplemented!(),
            "movefiled" => unimplemented!(),
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