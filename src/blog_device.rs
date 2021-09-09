extern crate redis;
extern crate r2d2_redis;

use crate::dashboard::QCommand;
use crate::device_trait::*;
use crate::news_payload_parser::*;
use crate::shikimori_scraper::run_parsing;

use redis::Commands;
use r2d2_redis::{RedisConnectionManager, r2d2};
use serde_json::Value as jsVal;
use serde_json::json;
use serde_json::from_str as js_from_str;

use std::ops::DerefMut;

type RedisPool = r2d2::Pool<RedisConnectionManager>;

#[derive(Clone)]
pub struct BlogDevice {
    conn_pool: RedisPool,
}

impl BlogDevice {
    pub fn new(db_config: &str, use_scraper: bool) -> Self {
        let manager = RedisConnectionManager::new(db_config).unwrap(); // I am a Blade Runner
        let pool = RedisPool::builder().build(manager).unwrap();
        if use_scraper {
            run_parsing(pool.clone());
        }
        BlogDevice { conn_pool: pool }
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
                        eprintln!("Error in parsing cmm: {:?}", err);
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

        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };

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

        Ok(json!({
            "template": "blog_post_list.hbs",
            "username": username,
            "post_count": buffer_v.len(),
            "posts": buffer_v,
            "can_post": 1 // TODO handle this correctrly on migrating to the redis
        }))
    }
}


impl DeviceRead for BlogDevice {
    fn read_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != "blogdev_read" {
            return Err("No access to this action".to_string());
        }

        match command {
            "getpost" => self.get_post(&query.username, &query.payload),
            _ => return Err(format!("Unknown for BlogDevice.read command: {}", command))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<jsVal, String> {
        if query.group != "rstatus" {
            return Err("No access to this action".to_string());
        }
        self.get_list_of_posts(&query.username)
    }
}


impl DeviceWrite for BlogDevice {
    fn write_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != "blogdev_write" {
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

        if query.group != "blogdev_request" {
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