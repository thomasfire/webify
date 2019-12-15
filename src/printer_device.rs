use crate::dashboard::QCommand;
use crate::device_trait::*;
use crate::config;
use crate::file_device::FileDevice;
use crate::io_tools;
use std::process::Command;
use std::fs::{remove_file, remove_dir_all, create_dir_all};
use diesel::{SqliteConnection, r2d2};
use diesel::r2d2::ConnectionManager;
use std::sync::{Arc, Mutex};
use std::collections::BTreeMap;
use crate::io_tools::exists;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone, Serialize, Deserialize)]
pub struct PrinterConfig {
    pub printer: String,
    pub storage: String,
}

#[derive(Clone, Debug)]
struct PrintRequest {
    id: u32,
    query: QCommand,
}

#[derive(Clone)]
pub struct PrinterDevice {
    config: PrinterConfig,
    filer: Arc<FileDevice>,
    queue: Arc<Mutex<BTreeMap<u32, PrintRequest>>>,
}

pub static PRINTER_CONFIG_PATH: &str = "printer_config.toml";

impl PrinterDevice {
    pub fn new(file_manager: Arc<FileDevice>) -> PrinterDevice {
        let config = config::read_config::<PrinterConfig>(PRINTER_CONFIG_PATH).unwrap_or(PrinterConfig
            { printer: "".to_string(), storage: "".to_string() });
        PrinterDevice { config, filer: file_manager.clone(), queue: Arc::new(Mutex::new(BTreeMap::new())) }
    }

    pub fn print_from_file(&self, filename: &str) -> Result<String, String> {
        let d = match Command::new("lp")
            .args(&["-d", &self.config.printer, &format!("{}", filename)]).output() {
            Ok(child) => child,
            Err(err) => return Err(format!("Error running the printing process (lp): {}", err)),
        };

        Ok(format!("Output: {};\nErrors: {};", String::from_utf8_lossy(&d.stdout), String::from_utf8_lossy(&d.stderr)))
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
        let filename = format!("{}/{}", self.config.storage, query.payload.split("/").last().unwrap_or("nonamefile"));
        println!("Cached to: {}", filename);
        match io_tools::write_bytes_to_file(&filename, data) {
            Ok(_) => Ok(filename),
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

    fn make_request(&self, query: &QCommand) -> Result<String, String> {
        self.queue.lock().map(|mut x| {
            let mut buff: u32 = 0;
            for y in 0..(256 * 256) {
                if !x.contains_key(&(y as u32)) {
                    x.insert(y, PrintRequest { query: query.clone(), id: y });
                    buff = y;
                    break;
                }
            }
            format!("OK, your id: {}", buff)
        }).map_err(|x| {
            format!("Internal error on making request: {:?}", x)
        })
    }

    fn delete_query(&self, ids: &str) -> Result<String, String> {
        let id: u32 = match ids.parse() {
            Ok(d) => d,
            Err(e) => return Err(format!("Error: wrong payload: {}", e))
        };
        self.queue.lock().map(|mut x| {
            match x.remove(&id) {
                Some(_d) => format!("OK, deleted {}", &id),
                None => format!("OK, there is no such request: {}", &id)
            }
        }).map_err(|x| {
            format!("Error on accessing the queue: {:?}", x)
        })
    }

    fn confirm_query(&self, ids: &str) -> Result<String, String> {
        let id: u32 = match ids.parse() {
            Ok(d) => d,
            Err(e) => return Err(format!("Error: wrong payload: {}", e))
        };
        let req = match self.queue.lock().map(|mut x| {
            match x.remove(&id) {
                Some(d) => Ok(d),
                None => Err(format!("Err, there is no such request: {}", &id))
            }
        }).map_err(|x| {
            format!("Error on accessing the queue: {:?}", x)
        }) {
            Ok(d) => match d {
                Ok(r) => r,
                Err(e) => return Err(format!("Error on unwrapping the result at `confirm_query`: {}", e))
            },
            Err(e) => return Err(format!("Error on confirming: {}", e)),
        };

        let path = match self.cache(&req.query) {
            Ok(d) => d,
            Err(e) => return Err(format!("Error on confirming and getting cached: {}", e))
        };

        self.print_from_file(&path)
    }

    fn get_list(&self) -> Result<String, String> {
        self.queue.lock()
            .map(|x| format!(r#"
            <table class="reqtable">
            <tr>
            <th>id</th>
            <th>username</th>
            <th>payload</th>
            <th></th>
            </tr>
            {}
            </table>"#, x.values().map(|t| {
                format!(r#"<tr>
                            <td>{}</td>
                            <td>{}</td>
                            <td>{}</td>
                        </tr>"#, t.id, t.query.username, t.query.payload)
            }).collect::<Vec<String>>().join("\n")))
            .map_err(|x| {
                format!("Error on getting the list: {:?}", x)
            })
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
              <input type="text" name="qtype" value="Q" class="qtype">
              <br>
              Group:<br>
              <input type="text" name="group" value="printer_request" class="group">
              <input type="hidden" name="username" value="{}" class="username">
              <br>
              Command:<br>
              <input type="text" name="command" value="print_file" class="command">
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
            "print_file" => self.print_from_file(&format!("{}/{}", self.config.storage, query.payload)),
            "cancel" => Self::cancel(&query.payload),
            "cache" => self.cache(&query),
            "cache_clear" => self.clear_cache(),
            _ => Err("Unknown command".to_string())
        }
    }
}


impl DeviceRequest for PrinterDevice {
    fn request_query(&self, query: &QCommand) -> Result<String, String> {
        if &query.group != "printer_request" {
            return Err("Error: wrong permissions".to_string());
        }
        match query.command.as_str() {
            "print_file" => self.make_request(query),
            _ => Err("Unknown command".to_string())
        }
    }
}

impl DeviceConfirm for PrinterDevice {
    fn confirm_query(&self, query: &QCommand) -> Result<String, String> {
        if &query.group != "printer_confirm" {
            return Err("Error: wrong permissions".to_string());
        }
        match query.command.as_str() {
            "confirm" => self.confirm_query(&query.payload),
            "list" => self.get_list(),
            _ => Err("Unknown command".to_string())
        }
    }

    fn dismiss_query(&self, query: &QCommand) -> Result<String, String> {
        if &query.group != "printer_confirm" {
            return Err("Error: wrong permissions".to_string());
        }
        match query.command.as_str() {
            "dismiss" => self.delete_query(&query.payload),
            _ => Err("Unknown command".to_string())
        }
    }
}