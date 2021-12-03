extern crate redis;
extern crate r2d2_redis;

use crate::dashboard::QCommand;
use crate::device_trait::*;
use crate::news_payload_parser::*;
use crate::shikimori_scraper::run_parsing;
use crate::database::Database;
use crate::devices::{Devices, Groups, DEV_GROUPS};

use redis::Commands;
use r2d2_redis::{RedisConnectionManager, r2d2};
use serde_json::Value as jsVal;
use serde_json::json;
use serde_json::from_str as js_from_str;
use log::error;
use chrono::Utc;

use std::ops::{DerefMut, Deref};
use std::sync::{RwLock, Arc};

type RedisPool = r2d2::Pool<RedisConnectionManager>;

#[derive(Clone)]
pub struct BlogDevice {
    conn_pool: RedisPool,
    database: Database,
    cache_list: Arc<RwLock<Vec<jsVal>>>,
    cache_last_synced: Arc<RwLock<i64>>,
}

impl BlogDevice {
    pub fn new(db_config: &str, database: &Database, use_scraper: bool) -> Self {
        let manager = RedisConnectionManager::new(db_config).unwrap(); // I am a Blade Runner
        let pool = RedisPool::builder().build(manager).unwrap();
        if use_scraper {
            run_parsing(pool.clone());
        }
        BlogDevice {
            conn_pool: pool,
            database: database.clone(),
            cache_list: Arc::new(RwLock::new(vec![])),
            cache_last_synced: Arc::new(RwLock::new(0)),
        }
    }

    fn new_post(&self, _username: &str, payload: &str) -> Result<jsVal, String> {
        let post: NewsPostParsed = match parse_post(payload) {
            Ok(val) => val,
            Err(err) => return Err(format!("Invalid post: {}", err))
        };
        let last_key = "ilast_post";
        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };

        let last_id: u32 = curr_conn.deref_mut().get(last_key).unwrap_or(0);
        let curr_id = last_id + 1;
        curr_conn.deref_mut().set(&format!("title_{}", curr_id), post.title)
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set(&format!("body_{}", curr_id), post.body)
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set(&format!("cmmcount_{}", curr_id), 0)
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set("ilast_post", curr_id).
            map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set("ilast_post_time", Utc::now().timestamp())
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        self.cache_list.write()
            .map_err(|err| format!("Error on clearing list of posts: {:?}", err))?
            .clear();
        *(self.cache_last_synced.write()
            .map_err(|err| format!("Error on clearing sync timestamp: {:?}", err))?) = 0;
        Ok(json!({
            "template": "simple_message.hbs",
            "message": "OK",
        }))
    }

    fn shownew_post(&self, username: &str, _payload: &str) -> Result<jsVal, String> {
        Ok(json!({"template": "blog_new_post.hbs", "username": username}))
    }

    fn get_post(&self, username: &str, payload: &str) -> Result<jsVal, String> {
        let post_id: u32 = payload.parse().map_err(|err| { format!("Couldn't parse the argument: {:?}", err) })?;
        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };
        let title: String = curr_conn.deref_mut().get(&format!("title_{}", post_id))
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        let body: String = curr_conn.deref_mut().get(&format!("body_{}", post_id))
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        let cmmcount: u32 = curr_conn.deref_mut().get(&format!("cmmcount_{}", post_id))
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        let cmms: Vec<String> = curr_conn.deref_mut().lrange(&format!("cmms_{}", post_id), 0, cmmcount as isize)
            .map_err(|err| { format!("Redis err: {:?}", err) })?;

        Ok(json!({
            "template": "blog_post_view.hbs",
            "username": username,
            "post_id": post_id,
            "title": title,
            "body": body,
            "cmmcount": cmmcount,
            "cmms": cmms.iter().map(|elem| {
                match js_from_str(&elem) {
                    Ok(data) => data,
                    Err(err) => {
                        error!("Error in parsing cmm: {:?}", err);
                        json!({})
                    }
                }
            }).collect::<jsVal>()
        }))
    }

    fn new_cmm(&self, username: &str, payload: &str) -> Result<String, String> {
        let cmm_parsed = parse_cmm(payload)?;
        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };
        let cmmcount: u32 = curr_conn.deref_mut().get(&format!("cmmcount_{}", cmm_parsed.post_id))
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set(&format!("cmmcount_{}", cmm_parsed.post_id), cmmcount + 1)
            .map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().rpush(&format!("cmms_{}", cmm_parsed.post_id),
                                    format!(r#"{{"username": "{}", "timestamp": "{}", "text": "{}"}}"#,
                                            username, cmm_parsed.date, cmm_parsed.text)).map_err(|err| { format!("Redis err: {:?}", err) })?;

        Ok("OK".to_string())
    }

    fn get_list_of_posts(&self, username: &str) -> Result<jsVal, String> {
        let last_key = "ilast_post";
        let last_post_time = "ilast_post_time";

        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };

        let last_real_time: i64 = curr_conn.deref_mut().get(last_post_time).unwrap_or(0) + 1;

        if last_real_time > self.cache_last_synced.read()
            .map_err(|err| format!("Error on reading sync timestamp: {:?}", err))?.deref().clone() {
            let last_id: u32 = curr_conn.deref_mut().get(last_key).unwrap_or(0) + 1;
            let mut buffer_v: Vec<jsVal> = vec![];
            buffer_v.reserve(last_id as usize);
            for x in 0..last_id {
                let title: String = curr_conn.deref_mut().get(&format!("title_{}", x)).unwrap_or("".to_string());
                if title.len() < 5 {
                    continue;
                }
                buffer_v.push(json!({
                    "id": x,
                    "title": title
                }));
            }
            self.cache_list.write()
                .map_err(|err| format!("Error on copying list of posts: {:?}", err))?
                .clone_from(&buffer_v);

            *(self.cache_last_synced.write()
                .map_err(|err| format!("Error on copying list of posts: {:?}", err))?) = Utc::now().timestamp();
        }

        let read_cache = self.cache_list.read()
            .map_err(|err| format!("Error on reading cached list: {:?}", err))?;

        Ok(json!({
            "template": "blog_post_list.hbs",
            "username": username,
            "post_count": read_cache.len(),
            "posts": read_cache.clone(),
            "can_post": self.database.has_access_to_group(username, DEV_GROUPS[Devices::Blog as usize][Groups::Write as usize].unwrap())
        }))
    }
}


impl DeviceRead for BlogDevice {
    fn read_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != DEV_GROUPS[Devices::Blog as usize][Groups::Read as usize].unwrap() {
            return Err("No access to this action".to_string());
        }

        match command {
            "getpost" => self.get_post(&query.username, &query.payload),
            _ => return Err(format!("Unknown for BlogDevice.read command: {}", command))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<jsVal, String> {
        if query.group != DEV_GROUPS[Devices::Zero as usize][Groups::RStatus as usize].unwrap() {
            return Err("No access to this action".to_string());
        }
        self.get_list_of_posts(&query.username)
    }
}


impl DeviceWrite for BlogDevice {
    fn write_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != DEV_GROUPS[Devices::Blog as usize][Groups::Write as usize].unwrap() {
            return Err("No access to this action".to_string());
        }

        match command {
            "createpost" => self.new_post(&query.username, &query.payload),
            "showcreatepost" => self.shownew_post(&query.username, &query.payload),
            _ => return Err(format!("Unknown for BlogDevice.read command: {}", command))
        }
    }
}


impl DeviceRequest for BlogDevice {
    fn request_query(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != DEV_GROUPS[Devices::Blog as usize][Groups::Request as usize].unwrap() {
            return Err("No access to this action".to_string());
        }


        match command {
            "createcmm" => self.new_cmm(&query.username, &query.payload),
            _ => return Err(format!("Unknown for BlogDevice.read command: {}", command))
        }.map(|data| {
            json!({
                "template": "simple_message.hbs",
                "message": data
            })
        })
    }
}

impl DeviceConfirm for BlogDevice {
    fn confirm_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}