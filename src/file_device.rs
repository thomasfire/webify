use crate::device_trait::*;

use diesel::{r2d2, SqliteConnection};
use diesel::r2d2::ConnectionManager;
use crate::dashboard::QCommand;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone)]
pub struct FileDevice {
    db_conn: Pool,
}

impl FileDevice {
    pub fn new(conn: &Pool) -> FileDevice {
        FileDevice { db_conn: conn.clone() }
    }
}


impl DeviceRead for FileDevice {
    fn read_data(&self, query: &QCommand) -> Result<String, String> {
        let command = query.command.as_str();

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
        unimplemented!()
    }
}

impl DeviceConfirm for FileDevice {
    fn confirm_query(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }

    fn dismiss_query(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }
}