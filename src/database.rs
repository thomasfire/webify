extern crate chrono;
extern crate crypto;
extern crate serde_json;

use std::collections::HashMap;
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

use crate::models::{Groups, UserAdd};
use crate::schema::*;

use self::crypto::digest::Digest;
use self::crypto::sha2::Sha256;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;


fn get_hash(text: &str) -> String {
    let mut hasher = Sha256::new();

    hasher.input_str(text);
    for _x in 0..512 {
        let hex = hasher.result_str();
        hasher.input_str(&hex);
    }

    return hasher.result_str();
}

pub fn get_user_groups(pool: Pool, username: &str) -> Result<Vec<String>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on init_db: {:?}", err)),
    };

    let res: Option<String> = match users::table.filter(users::columns::name.eq(username)).select(users::columns::groups).first(&connection) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error on getting user devices: {:?}", e);
            return Err(format!("Error on getting user devices: {:?}", e));
        }
    };

    match res {
        Some(gs) => return Ok(gs.split(",").map(|x| x.to_string()).collect()),
        None => return Ok(vec![])
    }
}


pub fn get_user_from_cookie(pool: Pool, cookie: &str) -> Result<String, String> {
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

pub fn get_user_devices(pool: Pool, devices_map: HashMap<String, String>, username: &str) -> Result<Vec<String>, String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on init_db: {:?}", err)),
    };

    let res: Option<String> = match users::table.filter(users::columns::name.eq(username)).select(users::columns::groups).first(&connection) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Error on getting user devices: {:?}", e);
            return Err(format!("Error on getting user devices: {:?}", e));
        }
    };

    match res {
        Some(gs) => Ok(gs.split(",").map(|x| devices_map.get(x)).filter(|x| match x {
            Some(_) => true,
            None => false,
        }).map(|x| x.unwrap().to_string()).collect::<Vec<String>>()),
        None => return Ok(vec![])
    }
}

pub fn insert_user(pool: Pool, username: &str, password: &str, groups: Option<&str>) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            println!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on insert_user (connection): {:?}", err)),
    };
    let new_user = UserAdd {
        name: username,
        password,
        groups,
    };

    match diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&connection) {
        Ok(_) => Ok(()),
        Err(err) => Err(format!("Error on inserting users (insert): {:?}", err))
    }
}

pub fn assign_cookie(pool: Pool, username: &str, cookie: &str) -> Result<(), String> {
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

pub fn has_access(pool: Pool, username: &str, group_name: &str) -> Result<bool, String> {
    let user_groups = match get_user_groups(pool, username) {
        Ok(data) => data,
        Err(err) => return Err(format!("Error on has_access on getting user devices: {:?}", err))
    };

    if user_groups.contains(&group_name.to_string()) {
        return Ok(true);
    }
    Ok(false)
}

pub fn on_init(pool: Pool) -> Result<HashMap<String, String>, String> {
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

pub fn get_connection(db_config: String) -> Result<Pool, String> {
    let manager = ConnectionManager::<SqliteConnection>::new(db_config);
    match Pool::builder().build(manager) {
        Ok(pool) => Ok(pool),
        Err(err) => Err(format!("Error on getting connection to DB: {:?}", err))
    }
}

pub fn init_db(db_config: String) -> Result<(), String> {
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
        groups TEXT ,
        wrong_attempts INTEGER null
    );
    CREATE TABLE history (
        id INTEGER primary key not null,
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