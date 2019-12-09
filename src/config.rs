extern crate toml;

use crate::io_tools;
use crate::database::{init_db, insert_group};
use crate::database::get_connection;
use crate::database::insert_user;
use serde::{Serialize, Deserialize};
use serde::de::DeserializeOwned;

#[derive(Serialize, Deserialize, Clone)]
pub struct Config {
    pub db_config: String,
    pub bind_address: String,
}

pub static default_config_path: &str = "config.toml";

/// Reads `config.toml` and returns Result with Users on Ok()
///
/// # Examples
///
/// ```rust
/// let users = read_config().unwrap();
/// ```
pub fn read_config<T: Serialize + DeserializeOwned + Clone>(conf_path: &str) -> Result<T, String>
{
    if !io_tools::exists(conf_path) {
        panic!("No `config.toml` file, run `$ teleprint --setup` ");
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
/// let config = Config {
///     token: String::from("ava24efsef345"),
///     printer: String::from("Your-Printer"),
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

/// Setups your Telegram/IMAP bots by command prompt
pub fn setup() {
    let bind_address = io_tools::read_std_line("Enter address to bind on: ");
    let db_config = io_tools::read_std_line("Enter sqlite path: ");

    match write_config::<Config>(Config {
        db_config: db_config.clone(),
        bind_address: bind_address.clone(),
    }, default_config_path) {
        Ok(_) => println!("Ok"),
        Err(err) => panic!("{:?}", err),
    };

    match init_db(&db_config) {
        Ok(_) => println!("Ok"),
        Err(err) => panic!("{:?}", err),
    };
}

pub fn add_user() {
    let username = io_tools::read_std_line("Enter new username: ");
    let password = io_tools::read_std_line("Enter new password: ");
    let groups = io_tools::read_std_line("Enter groups, separated by comma: ");

    let conf = match read_config::<Config>(default_config_path) {
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

pub fn add_group() {
    let g_name = io_tools::read_std_line("Enter new group name: ");
    let devices = io_tools::read_std_line("Enter devices, separated by comma: ");

    let conf = match read_config::<Config>(default_config_path) {
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

    match insert_group(&conn, &g_name, &devices) {
        Ok(_) => println!("Group was added successfully"),
        Err(e) => {
            eprintln!("Error on adding group to db: {}", e);
            return;
        }
    }
}