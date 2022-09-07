extern crate chrono;
extern crate crypto;
extern crate serde_json;
extern crate rand;
extern crate redis;
extern crate r2d2_redis;

use crate::models::{UserAdd, User, History, LineWebify, HistoryForm, StatEntry};
use crate::schema::*;
use crate::devices;

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
use log::{debug, error, info};
use secstr::SecStr;
use rustc_serialize::hex::ToHex;

use std::collections::{HashMap, BTreeSet};
use std::collections::btree_map::BTreeMap;
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
pub fn get_hash(text: &SecStr) -> String {
    const SALTY: &str = "af7rifgyurgfixf6547bzmU%^RFVYIjkszfhfzkdg64^&Izkdfh';jkkhuilyug25686hjbghfcrtyegbkhjgvjintyiiohdryujiytu";
    let initial_buff_sz = if 128 > text.unsecure().len() { 128 as usize } else { text.unsecure().len() };
    let mut buff_str = SecStr::new(vec![0; initial_buff_sz]);
    buff_str.unsecure_mut()[0..text.unsecure().len()].copy_from_slice(text.unsecure());
    for _x in 0..512 {
        let mut hasher = Sha256::new();
        hasher.input_str(SALTY);
        hasher.input(buff_str.unsecure());
        hasher.result(buff_str.unsecure_mut());
    }
    return buff_str.unsecure()[0..32].to_hex();
}

/// Do not use for passwords
pub fn get_fast_hash(text: &str) -> String {
    const SALTY: &str = "745otsryouf^I$&^T#FgYUKEhbfzdbfkxuryfuxf2347823gya";
    let mut buff_str = text.to_string();

    let mut hasher = Sha256::new();
    hasher.input_str(SALTY);
    hasher.input_str(&buff_str);
    buff_str = hasher.result_str();

    return buff_str;
}

/// Generates random token for the user
pub fn get_random_token() -> String {
    get_fast_hash(&(0..32).map(|_| random::<char>()).collect::<String>())
}

fn validate_password(password: &SecStr) -> Result<(), String> {
    let sz = password.unsecure().len();
    if sz < 8 || sz > 64 {
        return Err(format!("Unexpected password's length: {}, should be from 8 to 64", sz));
    }
    Ok(())
}

fn validate_username(username: &str) -> Result<(), String> {
    if username.len() < 4 || username.len() > 32 {
        return Err(format!("Unexpected username's length: {}, should be from 4 to 32", username.len()));
    }
    if !username.chars().all(|x| x.is_alphanumeric() || x == '_') {
        return Err(format!("Unexpected username's symbols: `{}`, allowed symbols are latin symbols (abcDEFG etc.), numbers and underline _", username));
    }
    Ok(())
}

impl Database {
    pub fn new(sql_conf: &str, redis_conf: &str) -> Result<Self, String> {
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
        validate_username(username)?;
        let mut redis_conn = self.redis_pool.get()
            .map_err(|err| format!("Error on getting the redis connection get_user_groups: {:?}", err))?;

        match redis_conn.deref_mut().get::<&str, String>(username) {
            Ok(data) => match js_from_str::<User>(&data) {
                Ok(usr_data) => return Ok(usr_data.groups.split(",").map(|x| x.to_string()).collect()),
                Err(_) => ()
            },
            Err(_) => ()
        };

        let mut connection = self.sql_pool.get().map_err(|err| format!("Error on get_user_groups: {:?}", err))?;

        let res: User = match users::table.filter(users::columns::name.eq(username))
            .first::<User>(&mut connection) {
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
        validate_username(username)?;
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
        validate_username(username)?;
        let groups = self.get_user_groups(username)?;

        let mapped_dev = self.mapped_devices.read()
            .map_err(|err| format!("Error on reading mapped device cache: {:?}", err))?;

        Ok(groups.iter().map(|x| mapped_dev.get(x).clone()).filter(|y| match y {
            Some(val) => if val.is_empty() { false } else { true },
            None => false,
        }).map(|x| x.unwrap_or(&"".to_string()).to_string()).collect::<BTreeSet<String>>().iter().cloned().collect::<Vec<String>>())
    }

    /// Returns vector of all users as they are represented in the database
    pub fn get_all_users(&self) -> Result<Vec<User>, String> {
        let mut connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on get_all_users (connection): {:?}", err)),
        };

        let users: Vec<User> = match users::table.load::<User>(&mut connection) {
            Ok(d) => d,
            Err(e) => return Err(format!("Error on loading users: {:?}", e)),
        };

        Ok(users)
    }

    /// Returns vector of all history records as they are represented in the database
    pub fn get_all_history(&self) -> Result<Vec<History>, String> {
        let mut connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on get_all_history (connection): {:?}", err)),
        };


        let hist: Vec<History> = match history::table.order(history::columns::id.desc()).limit(100).load::<History>(&mut connection) {
            Ok(d) => d,
            Err(e) => return Err(format!("Error on loading history: {:?}", e)),
        };

        Ok(hist)
    }

    /// Returns vector of all groups as they are represented in the database
    pub fn get_all_groups(&self) -> Result<BTreeMap<String, String>, String> {
        Ok(self.mapped_devices.read()
            .map_err(|err| format!("Error on reading mapped device cache: {:?}", err))?
            .iter().map(|(x, y)| (x.clone(), y.clone())).collect::<BTreeMap<String, String>>())
    }

    /// Inserts new user to the database, cookies are not assigned yet
    pub fn insert_user(&self, username: &str, password: &SecStr, groups: Option<&str>) -> Result<(), String> {
        insert_user(&self.sql_pool, username, password, groups)
    }

    /// Inserts new group to the databse
    pub fn insert_history(&self, username: &str, device: &str, command: &str, qtype: &str, rejected: i32) -> Result<(), String> {
        validate_username(username)?;
        let mut connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on insert_history (connection): {:?}", err)),
        };

        let entry = HistoryForm { username, device, command, qtype, rejected };

        match diesel::insert_into(history::table)
            .values(entry)
            .execute(&mut connection) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error on update_user_pass (update): {:?}", err))
        }
    }

    /// Updates password for the user to the databse
    pub fn update_user_pass(&self, username: &str, password: &SecStr) -> Result<(), String> {
        validate_password(password)?;
        validate_username(username)?;
        let mut connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on update_user_pass (connection): {:?}", err)),
        };
        self.delete_user_from_cache(username)?;

        match diesel::update(users::table.filter(users::columns::name.eq(username)))
            .set(users::columns::password.eq(get_hash(password)))
            .execute(&mut connection) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error on update_user_pass (update): {:?}", err))
        }
    }

    /// Updates password for the user to the databse
    pub fn update_users_ban(&self, usernames: &Vec<String>) -> Result<(), String> {
        let mut connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on update_user_ban (connection): {:?}", err)),
        };

        for username in usernames {
            validate_username(username)?;
            self.delete_user_from_cache(username)?;
        }

        match diesel::update(users::table.filter(users::columns::name.eq_any(usernames)))
            .set(users::columns::password.eq(""))
            .execute(&mut connection) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error on update_user_ban (update): {:?}", err))
        }
    }

    /// Writes groups for the user to the database
    pub fn update_user_group(&self, username: &str, group: &str) -> Result<(), String> {
        validate_username(username)?;
        let mut connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on update_user_group (connection): {:?}", err)),
        };

        self.delete_user_from_cache(username)?;
        match diesel::update(users::table.filter(users::columns::name.eq(username)))
            .set(users::columns::groups.eq(group))
            .execute(&mut connection) {
            Ok(_) => Ok(()),
            Err(err) => Err(format!("Error on update_user_group (update): {:?}", err))
        }
    }

    /// Writes cookies for the user to the database
    pub fn assign_cookie(&self, username: &str, cookie: &str) -> Result<(), String> {
        validate_username(username)?;
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
        validate_username(username)?;
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
        validate_username(username)?;
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
    pub fn validate_user(&self, username: &str, password: &SecStr) -> Result<bool, String> {
        validate_password(password)?;
        validate_username(username)?;
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
            let mut connection = match self.sql_pool.get() {
                Ok(conn) => {
                    debug!("Got connection");
                    conn
                }
                Err(err) => return Err(format!("Error on validate_user: {:?}", err)),
            };

            user = match users::table.filter(users::columns::name.eq(username))
                .first::<User>(&mut connection) {
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

        {
            let _ = redis_conn.deref_mut().set::<&str, String, String>(username, js_to_str(&got_user.get_content()).unwrap_or("".to_string()))
                .map_err(|err| error!("Error in caching to redis: {:?}", err));
            let _ = redis_conn.deref_mut().expire::<&str, usize>(username, REDIS_USER_EXPIRE)
                .map_err(|err| error!("Error in setting expire to redis: {:?}", err));
        }

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
        let mut devices_map = self.mapped_devices.write()
            .map_err(|err| format!("Error on writing mapped device: {:?}", err))?;
        for dev_i in 0..devices::DEVICES_LEN {
            for group_name_o in devices::DEV_GROUPS[dev_i] {
                match group_name_o {
                    Some(group_name) => devices_map.insert(group_name.to_string(), devices::DEV_NAMES[dev_i].to_string()),
                    None => continue,
                };
            }
        }
        Ok(())
    }

    pub fn load_stats_by_query(&self, query: &str) -> Result<Vec<StatEntry>, String> {
        let mut connection = match self.sql_pool.get() {
            Ok(conn) => {
                debug!("Got connection");
                conn
            }
            Err(err) => return Err(format!("Error on load_stats_by_query (connection): {:?}", err)),
        };

        let hist: Vec<StatEntry> = diesel::sql_query(query)
            .load(&mut connection)
            .expect("Query failed");

        Ok(hist)
    }
}


pub fn insert_user(pool: &SQLPool, username: &str, password: &SecStr, groups: Option<&str>) -> Result<(), String> {
    validate_password(password)?;
    validate_username(username)?;
    let mut connection = match pool.get() {
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
        password: &get_hash(password),
        groups: gs,
    };

    match diesel::insert_into(users::table)
        .values(&new_user)
        .execute(&mut connection) {
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

    let mut connection = match pool.get() {
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
        groups TEXT not null
    );
    CREATE TABLE history (
        id INTEGER primary key not null,
        username TEXT not null,
        device TEXT not null,
        command TEXT not null,
        qtype TEXT not null,
        rejected INTEGER not null DEFAULT 0,
        timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    );
    ") {
        Ok(_) => info!("DB has been initialized successfully"),
        Err(err) => return Err(format!("Error on init_db at execution: {:?}", err))
    };

    Ok(())
}