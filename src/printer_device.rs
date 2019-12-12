use crate::dashboard::QCommand;
use crate::device_trait::*;
use crate::config;
use crate::file_device::FileDevice;
use crate::io_tools;
use std::process::Command;
use std::fs::{remove_file, remove_dir_all, create_dir_all};
use diesel::{SqliteConnection, r2d2};
use diesel::r2d2::ConnectionManager;
use std::sync::Arc;
use crate::io_tools::exists;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct PrinterConfig {
    pub printer: String,
    pub storage: String,
}

#[derive(Clone)]
pub struct PrinterDevice {
    db_conn: Pool,
    config: PrinterConfig,
    filer: Arc<FileDevice>,
}

pub static PRINTER_CONFIG_PATH: &str = "printer_config.toml";

impl PrinterDevice {
    pub fn new(conn: &Pool, file_manager: Arc<FileDevice>) -> PrinterDevice {
        let config = config::read_config::<PrinterConfig>(PRINTER_CONFIG_PATH).unwrap_or(PrinterConfig
            { printer: "".to_string(), storage: "".to_string() });
        PrinterDevice { config, db_conn: conn.clone(), filer: file_manager.clone() }
    }

    pub fn print_from_file(&self, filename: &str) -> Result<String, String> {
        let printing_process = match Command::new("lp")
            .args(&["-d", &self.config.printer, &format!("{}/{}", self.config.storage, filename)]).spawn() {
            Ok(child) => child,
            Err(err) => return Err(format!("Error running the printing process (lp): {}", err)),
        };

        match printing_process.wait_with_output() {
            Ok(d) => Ok(format!("Output: {};\nErrors: {};", String::from_utf8_lossy(&d.stdout), String::from_utf8_lossy(&d.stderr))),
            Err(e) => Err(format!("Failed to wait for `lp`: {}", e)),
        }
    }

    /// Returns output of the `$ lpstat` command
    pub fn lpstat() -> String {
        match Command::new("lpstat")
            .output() {
            Ok(outp) => format!("lpstat:\n{}", String::from_utf8_lossy(&outp.stdout)),
            Err(err) => format!("lpstat error:\n{}", err),
        }
    }

    pub fn get_printers() -> String {
        match Command::new("lpstat").arg("-p")
            .output() {
            Ok(outp) => format!("lpstat -p:\n{}", String::from_utf8_lossy(&outp.stdout)),
            Err(err) => format!("lpstat error:\n{}", err),
        }
    }


    pub fn cancel(job: &str) -> Result<String, String> {
        let output = match Command::new("cancel")
            .arg(job)
            .output() {
            Ok(outp) => String::from(String::from_utf8_lossy(&outp.stdout)),
            Err(err) => return Err(format!("lprm error:\n {}", err)),
        };

        if output.len() < 3 {
            return Ok("Ok".to_string());
        }
        return Err(format!("Error on cancel: {}", output));
    }

    pub fn delete_file(&self, filename: &str) -> Result<String, String> {
        match remove_file(format!("{}/{}", self.config.storage, filename)) {
            Ok(_) => return Ok("Ok".to_string()),
            Err(err) => return Err(format!("Error: {:?}", err)),
        }
    }

    fn cache(&self, query: &QCommand) -> Result<String, String> {
        let data = match self.filer.get_file(&query.username, &query.payload) {
            Ok(d) => d,
            Err(e) => return Err(format!("Error on getting file for cache: {}", e))
        };
        if !exists(&self.config.storage) {
            match create_dir_all(&self.config.storage) {
                Ok(_) => (),
                Err(e) => return Err(format!("Error on creating the storage: {}", e)),
            };
        }
        match io_tools::write_bytes_to_file(&format!("{}/{}", self.config.storage, query.payload.split("/").last().unwrap_or("nonamefile")), data) {
            Ok(_) => Ok("OK".to_string()),
            Err(e) => Err(format!("Error on writing to the cache: {}", e))
        }
    }

    fn clear_cache(&self) -> Result<String, String> {
        match remove_dir_all(&self.config.storage) {
            Ok(_) => (),
            Err(e) => return Err(format!("Error on removing the directory: {}", e))
        };
        match create_dir_all(&self.config.storage) {
            Ok(_) => Ok("OK".to_string()),
            Err(e) => return Err(format!("Error on creating the storage: {}", e)),
        }
    }
}

impl DeviceRead for PrinterDevice {
    fn read_data(&self, query: &QCommand) -> Result<String, String> {
        if &query.group != "printer_read" {
            return Err("Error: wrong permission".to_string());
        }
        match query.command.as_str() {
            "lpstat" => Ok(Self::lpstat()),
            "printers" => Ok(Self::get_printers()),
            _ => Err("Unknown command".to_string()),
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<String, String> {
        if &query.group != "rstatus" {
            return Err("Error: wrong permission".to_string());
        }

        Ok(format!(r#"<div class="command_form">
        <form action="/dashboard/printer"  method="post" >
            <div class="command_f">
               QType:<br>
              <input type="text" name="qtype" value="R" class="qtype">
              <br>
              Group:<br>
              <input type="text" name="group" value="printer_read" class="group">
              <input type="hidden" name="username" value="{}" class="username">
              <br>
              Command:<br>
              <input type="text" name="command" value="" class="command">
              <br>
              <br>
              Payload:<br>
              <input type="text" name="payload" value="" class="payload">
              <br><br>
            </div>
              <input type="submit" value="Send" class="button">
        </form>
    </div><br>
    <div class="printer_info">
    {}
    </div>
    "#, query.username, Self::lpstat()))
    }
}

impl DeviceWrite for PrinterDevice {
    fn write_data(&self, query: &QCommand) -> Result<String, String> {
        if &query.group != "printer_write" {
            eprintln!("Wrong permission: {}, expected: printer_write", query.group);
            return Err("Error: wrong permissions".to_string());
        }

        match query.command.as_str() {
            "print_file" => self.print_from_file(&query.payload),
            "cancel" => Self::cancel(&query.payload),
            "cache" => self.cache(&query),
            "cache_clear" => self.clear_cache(),
            _ => Err("Unknown command".to_string())
        }
    }
}


impl DeviceRequest for PrinterDevice {
    fn request_query(&self, query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceConfirm for PrinterDevice {
    fn confirm_query(&self, query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }
}