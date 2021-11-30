extern crate chrono;
extern crate crypto;
extern crate serde_json;
extern crate rand;
extern crate redis;
extern crate r2d2_redis;

use crate::models::{Groups, UserAdd, User, History, GroupAdd, LineWebify, HistoryForm};
use crate::schema::*;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use rand::random;
use redis::Commands;
use r2d2_redis::{RedisConnectionManager, r2d2 as r2d2_red};
use diesel::connection::SimpleConnection;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use diesel::result::Error as dError;
use serde_json::from_str as js_from_str;
use serde_json::to_string as js_to_str;
use log::{debug, error, info, trace, warn};

use std::collections::{HashMap, BTreeSet};
use std::sync::{RwLock, Arc};
use std::ops::DerefMut;

type SQLPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;
type RedisPool = r2d2_red::Pool<RedisConnectionManager>;

const REDIS_USER_EXPIRE: usize = 3600 * 24;

#[derive(Clone)]
pub struct Database {
    sql_pool: SQLPool,
    redis_pool: RedisPool,
    cookie_cache: Arc<RwLock<HashMap<String, String>>>,
    attempts_cache: Arc<RwLock<HashMap<String, u32>>>,
    mapped_devices: Arc<RwLock<HashMap<String, String>>>,
}

/// Generates hash for the string. All password must go through this function
pub fn get_hash(text: &str) -> String {
    let mut buff_str = text.to_string();
    for _x in 0..512 {
        let mut hasher = Sha256::new();
        hasher.input_str(&buff_str);
        buff_str = hasher.result_str()
    }

    return buff_str;
}

/// Generates random token for the user
pub fn get_random_token() -> String {
    get_hash(&(0..32).map(|_| random::<char>()).collect::<String>())
}

impl Database {
    pub fn new(sql_conf: &str, redis_conf: &str) -> Result<Self, String> {
       // env_logger::init();
        debug!("databse new");
        warn!("databse new");
        info!("databse new");
        trace!("databse new");
        let redis_manager = RedisConnectionManager::new(redis_conf)
            .map_err(|err| { format!("Error on creating redis manager: {:?}", err) })?;
        let redis_pool = RedisPool::builder().build(redis_manager)
            .map_err(|err| { format!("Error on creating redis pool: {:?}", err) })?;

        let sql_manager = ConnectionManager::<SqliteConnection>::new(sql_conf);
        let sql_pool = SQLPool::builder().build(sql_manager)
            .map_err(|err| { format!("Error on creating sql pool: {:?}", err) })?;

        Ok(Database {
            redis_pool,
            sql_pool,
            cookie_cache: Arc::new(RwLock::new(HashMap::new())),
            attempts_cache: Arc::new(RwLock::new(HashMap::new())),
            mapped_devices: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Returns the list of user groups
    fn get_user_groups(&self, username: &str) -> Result<Vec<String>, String> {
        let mut redis_conn = self.redis_pool.get()
            .map_err(|err| format!("Error on getting the redis connection get_user_groups: {:?}", err))?;

        match redis_conn.deref_mut().get::<&str, String>(username) {
            Ok(data) => match js_from_str::<User>(&data) {
                Ok(usr_data) => return Ok(usr_data.groups.split(",").map(|x| x.to_string()).collect()),
                Err(_) => ()
            },
            Err(_) => ()
        };

        let connection = self.sql_pool.get().map_err(|err| format!("Error on get_user_groups: {:?}", err))?;

        let res: User = match users::table.filter(users::columns::name.eq(username))
            .first::<User>(&connection) {
            Ok(r) => r,
            Err(e) => {
                error!("Error on getting user devices (get_user_groups): {:?}", e);
                return Err(format!("Error on getting user devices: {:?}", e));
            }
        };

        let _ = redis_conn.deref_mut().set::<&str, String, String>(username, js_to_str(&res.get_content()).unwrap_or("".to_string()))
            .map_err(|err| error!("Error in caching to redis: {:?}", err));
        let _ = redis_conn.deref_mut().expire::<&str, usize>(username, REDIS_USER_EXPIRE)
            .map_err(|err| error!("Error in setting expire to redis: {:?}", err));

        Ok(res.groups.split(",").map(|x| x.to_string()).collect())
    }

    fn delete_user_from_cache(&self, username: &str) -> Result<(), String> {
        let mut redis_conn = self.redis_pool.get()
            .map_err(|err| format!("Error on getting the redis connection delete_user_from_cache: {:?}", err))?;
        redis_conn.deref_mut().del(username)
            .map_err(|err| format!("Error in deleting key in redis: {:?}", err))?;

        self.cookie_cache.write()
            .map_err(|err| format!("Error in deleting user from cookies: {:?}", err))?
            .retain(|_, val| val != username);

        self.attempts_cache.write()
            .map_err(|err| format!("Error in deleting user from attempts: {:?}", err))?
            .remove(username);

        Ok(())
    }

    /// Identifies user with the cookie
    pub fn get_user_from_cookie(&self, cookie: &str) -> Result<String, String> {
        match self.cookie_cache.read()
            .map_err(|err| format!("Error on reading cookie cache: {:?}", err))?
            .get(cookie) {
            Some(val) => return Ok(val.clone()),
            None => Err("No user for cookie".to_string())
        }
    }

    /// Returns the vector of all devices which are allowed for use by user
    pub fn get_user_devices(&self, username: &str) -> Result<Vec<String>, String> {
        let groups = self.get_user_groups(username)?;

        let mapped_dev = self.mapped_devices.read()
            .map_err(|err| format!("Error on reading mapped device cache: {:?}", err))?;

        Ok(groups.iter().map(|x| mapped_dev.get(x).clone()).filter(|y| match y {
            Some(_) => true,
            None => false,
        }).map(|x| x.unwrap_or(&"".to_string()).to_string()).collect::<BTreeSet<String>>().iter().cloned().collect::<Vec<String>>())
    }

    /// Returns vector of all users as they are represented in the database
    pub fn get_all_users(&self) -> Result<Vec<User>, String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
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

    /// Returns vector of all history records as they are represented in the database
    pub fn get_all_history(&self) -> Result<Vec<History>, String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on get_all_history (connection): {:?}", err)),
        };


        let hist: Vec<History> = match history::table.order(history::columns::id.desc()).limit(100).load::<History>(&connection) {
            Ok(d) => d,
            Err(e) => return Err(format!("Error on loading history: {:?}", e)),
        };

        Ok(hist)
    }

    /// Returns vector of all groups as they are represented in the database
    pub fn get_all_groups(&self) -> Result<Vec<Groups>, String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
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

    /// Inserts new user to the database, cookies are not assigned yet
    pub fn insert_user(&self, username: &str, password: &str, groups: Option<&str>) -> Result<(), String> {
        insert_user(&self.sql_pool, username, password, groups)
    }

    /// Inserts new group to the databse
    pub fn insert_group(&self, group_name: &str, device: &str) -> Result<(), String> {
        insert_group(&self.sql_pool, group_name, device)
    }

    /// Inserts new group to the databse
    pub fn insert_history(&self, username: &str, device: &str, command: &str, qtype: &str, rejected: i32) -> Result<(), String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on insert_history (connection): {:?}", err)),
        };

        let entry = HistoryForm {username, device, command, qtype, rejected};

        match diesel::insert_into(history::table)
            .values(entry)
            .execute(&connection) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error on update_user_pass (update): {:?}", err))
        }
    }

    /// Updates password for the user to the databse
    pub fn update_user_pass(&self, username: &str, password: &str) -> Result<(), String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on update_user_pass (connection): {:?}", err)),
        };
        self.delete_user_from_cache(username)?;

        match diesel::update(users::table.filter(users::columns::name.eq(username)))
            .set(users::columns::password.eq(get_hash(password)))
            .execute(&connection) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error on update_user_pass (update): {:?}", err))
        }
    }

    /// Writes groups for the user to the database
    pub fn update_user_group(&self, username: &str, group: &str) -> Result<(), String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on update_user_group (connection): {:?}", err)),
        };

        self.delete_user_from_cache(username)?;
        match diesel::update(users::table.filter(users::columns::name.eq(username)))
            .set(users::columns::groups.eq(group))
            .execute(&connection) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error on update_user_group (update): {:?}", err))
        }
    }

    /// Writes device for the group to the database
    pub fn update_group(&self, g_name: &str, devices: &str) -> Result<(), String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on update_group (connection): {:?}", err)),
        };

        match diesel::update(groups::table.filter(groups::columns::g_name.eq(g_name)))
            .set(groups::columns::devices.eq(devices))
            .execute(&connection) {
            Ok(_) => self.devices_reload(),
            Err(err) => Err(format!("Error on update_group (update): {:?}", err))
        }
    }

    /// Writes cookies for the user to the database
    pub fn assign_cookie(&self, username: &str, cookie: &str) -> Result<(), String> {
        self.cookie_cache.write()
            .map_err(|err| format!("Error on writing to cookie cache: {:?}", err))?
            .insert(cookie.to_string(), username.to_string());
        Ok(())
    }

    /// Removes cookie from database, making it impossible to log in
    pub fn remove_cookie(&self, cookie: &str) -> Result<(), String> {
        self.cookie_cache.write()
            .map_err(|err| format!("Error on removing cookie from cache: {:?}", err))?
            .remove(cookie);
        Ok(())
    }

    /// Returns whether user has access to the group
    pub fn has_access_to_group(&self, username: &str, group_name: &str) -> Result<bool, String> {
        let user_groups = match self.get_user_groups(username) {
            Ok(data) => data,
            Err(err) => return Err(format!("Error on has_access on getting user groups: {:?}", err))
        };

        if user_groups.contains(&group_name.to_string()) {
            return Ok(true);
        }
        Ok(false)
    }

    /// Returns whether user has access to the device, or not
    pub fn has_access_to_device(&self, username: &str, device: &str) -> Result<bool, String> {
        let user_dev = match self.get_user_devices(username) {
            Ok(data) => data,
            Err(err) => return Err(format!("Error on has_access on getting user devices: {:?}", err))
        };

        if user_dev.contains(&device.to_string()) {
            return Ok(true);
        }
        Ok(false)
    }

    /// Returns whether the user has correct credentials or not.
    /// Also manages the counter of unsuccessful logins. Currently it is allowed to make 10 wrong
    /// attempts for user before blocking the account. Every successful login resets the counter to 0.
    ///
    /// # Example
    /// ```rust
    /// use webify::database::{get_connection, validate_user};
    /// let conns = get_connection(&"database.db".to_string()).unwrap();
    /// assert!(validate_user(&conns, "thomasfire", "bestpasswort").unwrap());
    /// assert_ne!(validate_user(&conns, "eva", "badpasswort").unwrap());
    /// ```
    pub fn validate_user(&self, username: &str, password: &str) -> Result<bool, String> {
        let mut redis_conn = self.redis_pool.get()
            .map_err(|err| format!("Error on getting the redis connection get_user_groups: {:?}", err))?;

        let mut user: Option<User> = match redis_conn.deref_mut().get::<&str, String>(username) {
            Ok(data) => match js_from_str::<User>(&data) {
                Ok(usr_data) => Some(usr_data),
                Err(_) => None
            },
            Err(_) => None
        };

        if user.is_none() {
            let connection = match self.sql_pool.get() {
                Ok(conn) => {
                    debug!("Got connection");
                    conn
                }
                Err(err) => return Err(format!("Error on validate_user: {:?}", err)),
            };

            user = match users::table.filter(users::columns::name.eq(username))
                .first::<User>(&connection) {
                Ok(d) => Some(d),
                Err(e) => {
                    if e == dError::NotFound {
                        return Ok(false);
                    }
                    return Err(format!("Error on validating user: {:?}", e));
                }
            };
        }

        let got_user = match user {
            Some(d) => d,
            None => return Err(format!("Error no user: `{}`", username))
        };


        let wrongs = match self.attempts_cache.read()
            .map_err(|err| format!("Error on reading attempts: {:?}", err))?
            .get(username) {
            Some(d) => d.clone(),
            None => 0,
        };

        if wrongs >= 10 {
            return Ok(false);
        }

        let is_password_eq = got_user.password == get_hash(password);

        self.attempts_cache.write()
            .map_err(|err| format!("Error on writing attempts: {:?}", err))?
            .insert(username.to_string(), if is_password_eq { 0 } else { wrongs + 1 });
        Ok(is_password_eq)
    }

    /// Returns the devices map by their group name : {group_name: device_name};
    ///
    /// # Examples
    /// ```rust
    /// use webify::database::{get_connection, on_init};
    /// let conns = get_connection(&"database.db".to_string()).unwrap();
    /// let device_by_group = on_init(&conns).unwrap();
    /// assert_eq!(device_by_group.get("filer_read"), "filer");
    /// ```
    pub fn devices_reload(&self) -> Result<(), String> {
        let connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on assign cookie (connection): {:?}", err)),
        };

        let res: Vec<Groups> = match groups::table.load::<Groups>(&connection) {
            Ok(r) => r,
            Err(e) => {
                debug!("Error on getting user devices: {:?}", e);
                return Err(format!("Error on getting user devices: {:?}", e));
            }
        };

        {
            let mut devices_map = self.mapped_devices.write()
                .map_err(|err| format!("Error on writing mapped device: {:?}", err))?;
            for g_buff in &res {
                devices_map.insert(g_buff.g_name.clone(), g_buff.devices.clone());
            }
        }

        Ok(())
    }
}


pub fn insert_group(pool: &SQLPool, group_name: &str, device: &str) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            debug!("Got connection");
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


pub fn insert_user(pool: &SQLPool, username: &str, password: &str, groups: Option<&str>) -> Result<(), String> {
    let connection = match pool.get() {
        Ok(conn) => {
            debug!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on insert_user (connection): {:?}", err)),
    };

    let gs = match groups {
        Some(d) => d,
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

/// Returns the connection pool by the path to the database
///
/// # Example
/// ```rust
/// use webify::database::get_connection;
/// let connections = get_connection(&"database.db".to_string()).unwrap();
/// ```
pub fn get_connection(db_config: &String) -> Result<SQLPool, String> {
    let manager = ConnectionManager::<SqliteConnection>::new(db_config);
    match SQLPool::builder().build(manager) {
        Ok(pool) => Ok(pool),
        Err(err) => Err(format!("Error on getting connection to DB: {:?}", err))
    }
}

/// Writes initial data to the database: tables and list of groups
///
/// # Example
/// ```rust
/// use webify::database::init_db;
/// init_db(&"database.db".to_string()).unwrap();
/// ```
pub fn init_db(db_config: &String) -> Result<(), String> {
    let r_pool = get_connection(db_config);
    let pool = match r_pool {
        Ok(conn) => {
            debug!("Connection established");
            conn
        }
        Err(err) => return Err(format!("Error on init_db: {}", err)),
    };

    let connection = match pool.get() {
        Ok(conn) => {
            debug!("Got connection");
            conn
        }
        Err(err) => return Err(format!("Error on init_db: {:?}", err)),
    };

    match connection.batch_execute("
    CREATE TABLE users (
        id INTEGER primary key not null,
        name TEXT not null,
        password TEXT not null,
        groups TEXT not null,
        wrong_attempts INTEGER null
    );
    CREATE TABLE history (
        id INTEGER primary key not null,
        username TEXT not null,
        device TEXT not null,
        command TEXT not null,
        rejected INTEGER not null DEFAULT 0,
        timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    CREATE TABLE groups (
        id INTEGER primary key not null,
        g_name TEXT not null,
        devices TEXT not null
    );
    INSERT INTO groups (id, g_name, devices) VALUES (1, 'filer_read', 'filer');
    INSERT INTO groups (id, g_name, devices) VALUES (2, 'filer_write', 'filer');
    INSERT INTO groups (id, g_name, devices) VALUES (3, 'root_write', 'root');
    INSERT INTO groups (id, g_name, devices) VALUES (4, 'root_read', 'root');
    INSERT INTO groups (id, g_name, devices) VALUES (5, 'printer_read', 'printer');
    INSERT INTO groups (id, g_name, devices) VALUES (6, 'printer_write', 'printer');
    INSERT INTO groups (id, g_name, devices) VALUES (7, 'printer_request', 'printer');
    INSERT INTO groups (id, g_name, devices) VALUES (8, 'printer_confirm', 'printer');
    INSERT INTO groups (id, g_name, devices) VALUES (9, 'blogdev_write', 'blogdev');
    INSERT INTO groups (id, g_name, devices) VALUES (10, 'blogdev_request', 'blogdev');
    INSERT INTO groups (id, g_name, devices) VALUES (11, 'blogdev_read', 'blogdev');
    ") {
        Ok(_) => info!("DB has been initialized successfully"),
        Err(err) => return Err(format!("Error on init_db at execution: {:?}", err))
    };

    Ok(())
}