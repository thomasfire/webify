extern crate toml;

use crate::io_tools;
use crate::database::{init_db, get_connection, insert_user};
use crate::printer_device::{PrinterDevice, PRINTER_CONFIG_PATH, PrinterConfig};
use crate::devices::list_all_groups;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub db_config: String,
    pub bind_address: String,
    pub redis_config: String,
    pub redis_cache: String,
    pub use_scraper: bool,
    pub general_stat_period_s: u32,
    pub cross_user_stat_period_s: u32,
    pub period_to_request_s: u32,
    pub autoban_period_s: u32,
    pub autoban_anomaly_factor: f64,
}

pub static DEFAULT_CONFIG_PATH: &str = "config.toml";

/// Reads `config.toml` and returns Result with Users on Ok()
///
/// # Examples
///
/// ```rust
/// use webify::config::{read_config, Config};
/// let users = read_config::<Config>("config.toml").unwrap();
/// ```
pub fn read_config<T: Serialize + DeserializeOwned + Clone>(conf_path: &str) -> Result<T, String>
{
    if !io_tools::exists(conf_path) {
        panic!("No `config.toml` file, run `$ webify --setup` ");
    }
    let config_str = match io_tools::read_str(conf_path) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("Error on reading the config: {:?}", err);
            return Err("Error on reading the config".to_string());
        }
    };
    let config: T = match toml::from_str(&config_str) {
        Ok(value) => value,
        Err(err) => {
            println!("Something goes wrong while reading the users: {}", err);
            return Err(format!("{:?}", err));
        }
    };

    Ok(config)
}


/// Writes Config to the `config.toml`, returns Result
///
/// # Examples
///
/// ```rust
/// use webify::config::Config;
/// let config = Config {
///     db_config: String::from("database.db"),
///     bind_address: String::from("127.0.0.1:2280"),
///     redis_config: String::from("redis://127.0.0.1:6379/"),
///     redis_cache: String::from("redis://127.0.0.1:6380/"),
///     general_stat_period_s: 3600,
///     cross_user_stat_period_s: 3600,
///     period_to_request_s: 2592000,
///     autoban_period_s: 24*3600,
///     autoban_anomaly_factor: 8.0,
///     use_scraper: true
/// };
/// write_database(config).unwrap();
/// ```
pub fn write_config<T: Serialize + DeserializeOwned + Clone>(config: T, conf_path: &str) -> Result<(), String> {
    let conf_str = match toml::to_string(&config) {
        Ok(value) => value,
        Err(err) => {
            println!("Something went wrong while parsing the config: {}", err);
            panic!("{}", err);
        }
    };


    match io_tools::write_to_file(conf_path, conf_str) {
        Ok(_) => return Ok(()),
        Err(err) => {
            println!("An error occured while writing to the config: {}", err);
            return Err(format!("{:?}", err));
        }
    };
}

/// Asks all necessary data for configuring the server and writes proper config
pub fn setup() {
    let bind_address = io_tools::read_std_line("Enter address to bind on: ");
    let db_config = io_tools::read_std_line("Enter sqlite path: ");
    let redis_config = io_tools::read_std_line("Enter redis config URL (eg redis://127.0.0.1:6379/): ");
    let redis_cache = io_tools::read_std_line("Enter redis cache URL (eg redis://127.0.0.1:6380/): ");
    let use_scraper = io_tools::read_std_line("Use scraper to fetch news from external resources? (true/false) ").parse::<bool>().unwrap();
    let general_stat_period = io_tools::read_std_line("General stats period in seconds (0 to disable): ").parse::<u32>().unwrap();
    let cross_user_stat_period = io_tools::read_std_line("Per user stats period in seconds (0 to disable): ").parse::<u32>().unwrap();
    let period_to_request = io_tools::read_std_line("Period of the statistics (0 to disable): ").parse::<u32>().unwrap();
    let autoban_s = io_tools::read_std_line("Period of the autoban worker (0 to disable): ").parse::<u32>().unwrap();
    let anomaly_f = io_tools::read_std_line("Factor for detecting anomalies via autoban (0 to disable): ").parse::<f64>().unwrap();

    println!("\nHere is your printers:\n{}\n", PrinterDevice::get_printers());
    let m_printer = io_tools::read_std_line("Enter name of the printer: ");
    let m_storage = io_tools::read_std_line("Enter path to the printer storage: ");

    match write_config::<Config>(Config {
        db_config: db_config.clone(),
        bind_address: bind_address.clone(),
        redis_config: redis_config.clone(),
        redis_cache: redis_cache.clone(),
        general_stat_period_s: general_stat_period,
        cross_user_stat_period_s: cross_user_stat_period,
        period_to_request_s: period_to_request,
        autoban_period_s: autoban_s,
        autoban_anomaly_factor: anomaly_f,
        use_scraper
    }, DEFAULT_CONFIG_PATH) {
        Ok(_) => println!("Ok"),
        Err(err) => panic!("{:?}", err),
    };

    match write_config::<PrinterConfig>(PrinterConfig {
        storage: m_storage,
        printer: m_printer,
    }, PRINTER_CONFIG_PATH) {
        Ok(_) => println!("Printer Ok"),
        Err(err) => panic!("{:?}", err),
    };

    match init_db(&db_config) {
        Ok(_) => println!("Ok"),
        Err(err) => panic!("{:?}", err),
    };
}


pub fn add_root_user() {
    let username = io_tools::read_std_line("Enter root username: ");
    let password = io_tools::read_std_line("Enter root password: ");

    let conf = match read_config::<Config>(DEFAULT_CONFIG_PATH) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on reading config: {}", e);
            return;
        }
    };

    let conn = match get_connection(&conf.db_config) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on connecting to db: {}", e);
            return;
        }
    };

    match insert_user(&conn, &username, &password, Some(&list_all_groups().join(","))) {
        Ok(_) => println!("User was added successfully"),
        Err(e) => {
            eprintln!("Error on adding user to db: {}", e);
            return;
        }
    }
}

/// Adds user to the previously configured database
pub fn add_user() {
    let username = io_tools::read_std_line("Enter new username: ");
    let password = io_tools::read_std_line("Enter new password: ");
    let groups = io_tools::read_std_line("Enter groups, separated by comma: ");

    let conf = match read_config::<Config>(DEFAULT_CONFIG_PATH) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on reading config: {}", e);
            return;
        }
    };

    let conn = match get_connection(&conf.db_config) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error on connecting to db: {}", e);
            return;
        }
    };

    match insert_user(&conn, &username, &password, Some(&groups)) {
        Ok(_) => println!("User was added successfully"),
        Err(e) => {
            eprintln!("Error on adding user to db: {}", e);
            return;
        }
    }
}
