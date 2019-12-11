use crate::dashboard::QCommand;
use crate::device_trait::*;
use crate::config;
use std::process::Command;
use std::fs::remove_file;
use diesel::{SqliteConnection, r2d2};
use diesel::r2d2::ConnectionManager;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct PrinterConfig {
    pub printer: String,
    pub storage: String,
}

#[derive(Clone)]
pub struct PrinterDevice {
    db_conn: Pool,
    // TODO
    config: PrinterConfig,
}

pub static PRINTER_CONFIG_PATH: &str = "printer_config.toml";

impl PrinterDevice {
    pub fn new(conn: &Pool) -> PrinterDevice {
        let config = config::read_config::<PrinterConfig>(PRINTER_CONFIG_PATH).unwrap_or(PrinterConfig
            { printer: "".to_string(), storage: "".to_string() });
        PrinterDevice { config, db_conn: conn.clone() }
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
}

impl DeviceRead for PrinterDevice {
    fn read_data(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }

    fn read_status(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }
}

impl DeviceWrite for PrinterDevice {
    fn write_data(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }
}


impl DeviceRequest for PrinterDevice {
    fn request_query(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }
}

impl DeviceConfirm for PrinterDevice {
    fn confirm_query(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }

    fn dismiss_query(&self, query: &QCommand) -> Result<String, String> {
        unimplemented!()
    }
}