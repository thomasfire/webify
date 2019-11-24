extern crate chrono;
extern crate crypto;
extern crate serde_json;
extern crate rand;

use std::collections::HashMap;

use rand::random;
use std::error::Error;

use chrono::NaiveDateTime;
use diesel::connection::SimpleConnection;
#[cfg(test)]
use diesel::debug_query;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
#[cfg(test)]
use diesel::sqlite::Sqlite;
use diesel::result::Error as dError;

use crate::models::{Groups, UserAdd, User, History, GroupAdd};
use crate::schema::*;

use self::crypto::digest::Digest;
use self::crypto::sha2::Sha256;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;


fn get_hash(text: &str) -> String {
    let mut buff_str = text.to_string();
    for _x in 0..512 {
        let mut hasher = Sha256::new();
        hasher.input_str(&buff_str);
        buff_str = hasher.result_str()
    }

    return buff_str;
}

pub fn get_random_token() -> String {
    get_hash(&(0..32).map(|_| random::<char>()).collect::<String>())
}

pub fn get_user_groups(pool: &Pool, username: &str) -> Result<Vec<String>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on init_db: {:?}", err)),
    };

    let res: String = match users::table.filter(users::columns::name.eq(username))
        .select(users::columns::groups)
        .first(&connection) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error on getting user devices (get_user_groups): {:?}", e);
            return Err(format!("Error on getting user devices: {:?}", e));
        }
    };

    Ok(res.split(",").map(|x| x.to_string()).collect())
}


pub fn get_user_from_cookie(pool: &Pool, cookie: &str) -> Result<String, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on get_user_from_cookie: {:?}", err)),
    };

    match users::table.filter(users::columns::cookie.eq(cookie)).select(users::columns::name).first(&connection) {
        Ok(r) => Ok(r),
        Err(e) => {
            eprintln!("Error on getting user from cookie: {:?}", e);
            return Err(format!("Error on getting user from cookie: {:?}", e));
        }
    }
}

pub fn get_user_devices(pool: &Pool, devices_map: &HashMap<String, String>, username: &str) -> Result<Vec<String>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on get_user_devices: {:?}", err)),
    };

    let res: String = match users::table.filter(users::columns::name.eq(username)).select(users::columns::groups).first(&connection) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error on getting user devices (get_user_devices): {:?}", e);
            return Err(format!("Error on getting user devices: {:?}", e));
        }
    };

    Ok(res.split(",").map(|x| devices_map.get(x)).filter(|x| match x {
        Some(_) => true,
        None => false,
    }).map(|x| x.unwrap_or(&"".to_string()).to_string()).collect::<Vec<String>>())
}

pub fn get_all_users(pool: &Pool) -> Result<Vec<User>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on get_all_users (connection): {:?}", err)),
    };


    let users: Vec<User> = match users::table.load::<User>(&connection) {
        Ok(d) => d,
        Err(e) => return Err(format!("Error on loading users: {:?}", e)),
    };

    Ok(users)
}


pub fn get_all_history(pool: &Pool) -> Result<Vec<History>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on get_all_history (connection): {:?}", err)),
    };


    let hist: Vec<History> = match history::table.order(history::columns::id.desc()).load::<History>(&connection) {
        Ok(d) => d,
        Err(e) => return Err(format!("Error on loading history: {:?}", e)),
    };

    Ok(hist)
}


pub fn get_all_groups(pool: &Pool) -> Result<Vec<Groups>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on get_all_groups (connection): {:?}", err)),
    };


    let group: Vec<Groups> = match groups::table.load::<Groups>(&connection) {
        Ok(d) => d,
        Err(e) => return Err(format!("Error on loading groups: {:?}", e)),
    };

    Ok(group)
}

pub fn insert_user(pool: &Pool, username: &str, password: &str, groups: Option<&String>) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on insert_user (connection): {:?}", err)),
    };

    let gs = match groups {
        Some(d) => d.as_str(),
        None => ""
    };
    let new_user = UserAdd {
        name: username,
        password: &get_hash(&password),
        groups: gs,
    };

    match diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&connection) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Error on inserting users (insert): {:?}", err))
    }
}

pub fn insert_group(pool: &Pool, group_name: &str, device: &str) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on insert_group (connection): {:?}", err)),
    };

    let new_group = GroupAdd {
        g_name: group_name,
        devices: device,
    };

    match diesel::insert_into(groups::table)
        .values(&new_group)
        .execute(&connection) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Error on inserting groups (insert): {:?}", err))
    }
}


pub fn update_user_pass(pool: &Pool, username: &str, password: &str) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on insert_user (connection): {:?}", err)),
    };

    match diesel::update(users::table.filter(users::columns::name.eq(username)))
        .set(users::columns::password.eq(get_hash(password)))
        .execute(&connection) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Error on inserting users (insert): {:?}", err))
    }
}

pub fn update_user_group(pool: &Pool, username: &str, group: &str) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on insert_user (connection): {:?}", err)),
    };

    match diesel::update(users::table.filter(users::columns::name.eq(username)))
        .set(users::columns::groups.eq(group))
        .execute(&connection) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Error on inserting users (insert): {:?}", err))
    }
}


pub fn update_group(pool: &Pool, g_name: &str, devices: &str) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on insert_user (connection): {:?}", err)),
    };

    match diesel::update(groups::table.filter(groups::columns::g_name.eq(g_name)))
        .set(groups::columns::devices.eq(devices))
        .execute(&connection) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Error on inserting users (insert): {:?}", err))
    }
}

pub fn assign_cookie(pool: &Pool, username: &str, cookie: &str) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on assign cookie (connection): {:?}", err)),
    };

    match diesel::update(users::table.filter(users::columns::name.eq(username)))
        .set(users::columns::cookie.eq(cookie))
        .execute(&connection) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Error on assign cookie (update): {:?}", err))
    }
}

pub fn has_access(pool: &Pool, username: &str, group_name: &str) -> Result<bool, String> {
    let user_groups = match get_user_groups(pool, username) {
        Ok(data) => data,
        Err(err) => return Err(format!("Error on has_access on getting user devices: {:?}", err))
    };

    if user_groups.contains(&group_name.to_string()) {
        return Ok(true);
    }
    Ok(false)
}

pub fn validate_user(pool: &Pool, username: &str, password: &str) -> Result<bool, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on validate_user: {:?}", err)),
    };

    let (b_id, b_password, b_wrongs) = match users::table.filter(users::columns::name.eq(username))
        .select((users::columns::id, users::columns::password, users::columns::wrong_attempts))
        .first::<(i32, String, Option<i32>)>(&connection) {
        Ok(d) => d,
        Err(e) => {
            if e == dError::NotFound {
                return Ok(false);
            }
            return Err(format!("Error on validating user: {:?}", e));
        }
    };

    let wrongs = match b_wrongs {
        Some(d) => d,
        None => 0,
    };

    if wrongs >= 10 {
        return Ok(false);
    }

    if b_password == get_hash(password) {
        match diesel::update(users::table.filter(users::columns::id.eq(b_id)))
            .set(users::columns::wrong_attempts.eq(0))
            .execute(&connection) {
            Ok(_) => return Ok(true),
            Err(e) => {
                eprintln!("Error on resetting attempts: {:?}", e);
                return Err(format!("Error on resetting attempts: {:?}", e));
            }
        }
    }


    match diesel::update(users::table.filter(users::columns::id.eq(b_id)))
        .set(users::columns::wrong_attempts.eq(wrongs + 1))
        .execute(&connection) {
        Ok(_) => return Ok(false),
        Err(e) => {
            eprintln!("Error on resetting attempts: {:?}", e);
            return Err(format!("Error on resetting attempts: {:?}", e));
        }
    }
}


pub fn on_init(pool: &Pool) -> Result<HashMap<String, String>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on assign cookie (connection): {:?}", err)),
    };

    let res: Vec<Groups> = match groups::table.load::<Groups>(&connection) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error on getting user devices: {:?}", e);
            return Err(format!("Error on getting user devices: {:?}", e));
        }
    };

    let mut devices_map: HashMap<String, String> = HashMap::new();

    for g_buff in &res {
        devices_map.insert(g_buff.g_name.clone(), g_buff.devices.clone());
    }

    Ok(devices_map)
}

pub fn get_connection(db_config: &String) -> Result<Pool, String> {
    let manager = ConnectionManager::<SqliteConnection>::new(db_config);
    match Pool::builder().build(manager) {
        Ok(pool) => Ok(pool),
        Err(err) => Err(format!("Error on getting connection to DB: {:?}", err))
    }
}

pub fn init_db(db_config: &String) -> Result<(), String> {
    let r_pool = get_connection(db_config);
    let pool = match r_pool {
        Ok(conn) => {
            println!("Connection established");
            conn
        }
        Err(err) => return Err(format!("Error on init_db: {}", err)),
    };

    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on init_db: {:?}", err)),
    };

    match connection.batch_execute("
    CREATE TABLE users (
        id INTEGER primary key not null,
        name TEXT not null,
        password TEXT not null,
        cookie TEXT null,
        groups TEXT not null,
        wrong_attempts INTEGER null
    );
    CREATE TABLE history (
        id INTEGER primary key not null,
        username TEXT null,
        get_query TEXT not null,
        timestamp TIMESTAMP not null
    );
    CREATE TABLE groups (
        id INTEGER primary key not null,
        g_name TEXT not null,
        devices TEXT not null
    )
    ") {
        Ok(_) => println!("DB has been initialized successfully"),
        Err(err) => return Err(format!("Error on init_db at execution: {:?}", err))
    };

    Ok(())
}