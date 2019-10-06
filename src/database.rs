extern crate chrono;


extern crate serde_json;

use std::error::Error;

use chrono::NaiveDateTime;
#[cfg(test)]
use diesel::debug_query;
use diesel::insert_into;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
#[cfg(test)]
use diesel::sqlite::Sqlite;



use diesel::connection::SimpleConnection;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

mod schema {
    table! {
        users {
            id -> Integer,
            name -> Text,
            password -> Text,
            groups -> Nullable<Text>,
        }
    }
    table! {
        history {
            id -> Integer,
            get_query -> Text,
            timestamp -> Timestamp,
        }
    }
}

use schema::history;
use schema::users;

#[derive(Queryable, PartialEq, Debug)]
struct User {
    id: u32,
    name: String,
    password: String,
    groups: Option<String>,
}


#[derive(Deserialize, Insertable)]
#[table_name = "users"]
pub struct UserForm<'a> {
    name: &'a str,
    password: &'a str,
    groups: Option<&'a str>,
}


#[derive(Queryable, PartialEq, Debug)]
struct History {
    id: u32,
    get_query: String,
    timestamp: NaiveDateTime,
}

#[derive(Deserialize, Insertable)]
#[table_name = "history"]
pub struct HistoryForm<'a> {
    get_query: &'a str
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
        id INTEGER primary key,
        name TEXT not null,
        password TEXT not null,
        groups TEXT null
    );
    CREATE TABLE history (
        id INTEGER primary key,
        get_query TEXT not null,
        timestamp TIMESTAMP
    );
    ") {
        Ok(_) => println!("DB has been initialized successfully"),
        Err(err) => return Err(format!("Error on init_db at execution: {:?}", err))
    };

    Ok(())
}