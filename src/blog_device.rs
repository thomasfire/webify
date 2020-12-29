extern crate redis;
extern crate r2d2_redis;

use crate::dashboard::QCommand;
use crate::device_trait::*;
use crate::news_payload_parser::*;
use redis::Commands;

use r2d2_redis::{RedisConnectionManager, r2d2};
use std::ops::DerefMut;


type RedisPool = r2d2::Pool<RedisConnectionManager>;

#[derive(Clone)]
pub struct BlogDevice {
    conn_pool: RedisPool
}

impl BlogDevice {
    pub fn new(db_config: &str) -> Self {
        let manager = RedisConnectionManager::new(db_config).unwrap(); // I am a Blade Runner
        let pool = RedisPool::builder().build(manager).unwrap();
        BlogDevice { conn_pool: pool }
    }

    fn new_post(&self, _username: &str, payload: &str) -> Result<String, String> {
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
        curr_conn.deref_mut().set(&format!("title_{}", curr_id), post.title).map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set(&format!("body_{}", curr_id), post.body).map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set(&format!("cmmcount_{}", curr_id), 0).map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set("ilast_post", curr_id).map_err(|err| { format!("Redis err: {:?}", err) })?;
        Ok(format!("OK"))
    }

    fn shownew_post(&self, username: &str, _payload: &str)-> Result<String, String> {
        Ok(format!(r#"
        <div class="postnewpost">
                      <script>
                        function send_post() {{
                            let title_t = document.getElementById('payload_post_title');
                            let bod_t = document.getElementById('payload_post_body');
                            document.getElementById('payload_inpt').value = "<title>" + title_t.value + "</title><body>" + bod_t.value + "</body>";
                            document.getElementById('post_sender').submit();
                        }}
                    </script>

<textarea name="title" class="payload" id="payload_post_title" form="">Your title here...</textarea>
<textarea name="body" class="payload" id="payload_post_body" form="">Your body here...</textarea>

<form action="/dashboard/blogdev" method="post" id="post_sender">
    <div class="command_f">
        <input type="hidden" name="qtype" value="W" class="qtype">
        <input type="hidden" name="group" value="blogdev_write" class="group">
        <input type="hidden" name="username" value="{}" class="username">
        <input type="hidden" name="command" value="createpost" class="command">
        <input type="hidden" name="payload" class="payload" id="payload_inpt">
    </div>
    <a onclick="send_post();" class="post_sender">Send Post</a>
</form>
</div>
        "#, username))
    }

    fn get_post(&self, username: &str, payload: &str) -> Result<String, String> {
        let post_id: u32 = payload.parse().map_err(|err| { format!("Couldn't parse the argument: {:?}", err) })?;
        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };
        let title: String = curr_conn.deref_mut().get(&format!("title_{}", post_id)).map_err(|err| { format!("Redis err: {:?}", err) })?;
        let body: String = curr_conn.deref_mut().get(&format!("body_{}", post_id)).map_err(|err| { format!("Redis err: {:?}", err) })?;
        let cmmcount: u32 = curr_conn.deref_mut().get(&format!("cmmcount_{}", post_id)).map_err(|err| { format!("Redis err: {:?}", err) })?;
        let cmms: Vec<String> = curr_conn.deref_mut().lrange(&format!("cmms_{}", post_id), 0, cmmcount as isize).map_err(|err| { format!("Redis err: {:?}", err) })?;
        let cmms_block = format!(r#"<div class="cmmblock">
            <div class="cmmcounter">{}</div>
            {}
        </div>"#, cmmcount, cmms.iter().map(|elem| { format!("<div class=\"cmmitem\">{}</div>", elem) }).collect::<Vec<String>>().join("\n"));

        Ok(format!(r#"<div class="posttitle">{}</div>
                      <div class="postbody">{}</div>
                      <div class="postbottom">{}</div>
                      <div class="postnewcmm">
                      <script>
                        function send_cmm() {{
                            let cmm_t = document.getElementById('payload_cmm_new');
                            document.getElementById('payload_inpt').value = "<id>{}</id><text>" + cmm_t.value + "</text>";
                            document.getElementById('cmm_sender').submit();
                        }}
                    </script>

<textarea name="payload_t" class="payload_t" id="payload_cmm_new" form="">Your comment here...</textarea>

<form action="/dashboard/blogdev" method="post" id="cmm_sender">
    <div class="command_f">
        <input type="hidden" name="qtype" value="Q" class="qtype">
        <input type="hidden" name="group" value="blogdev_request" class="group">
        <input type="hidden" name="username" value="{}" class="username">
        <input type="hidden" name="command" value="createcmm" class="command">
        <input type="hidden" name="payload" class="payload" id="payload_inpt">
    </div>
    <a onclick="send_cmm();" class="cmmsender">Send Comment</a>
</form>
</div>
                      "#, title, body, cmms_block, post_id, username))
    }

    fn new_cmm(&self, username: &str, payload: &str) -> Result<String, String> {
        let cmm_parsed = parse_cmm(payload)?;
        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };
        let cmmcount: u32 = curr_conn.deref_mut().get(&format!("cmmcount_{}", cmm_parsed.post_id)).map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().set(&format!("cmmcount_{}", cmm_parsed.post_id), cmmcount + 1).map_err(|err| { format!("Redis err: {:?}", err) })?;
        curr_conn.deref_mut().rpush(&format!("cmms_{}", cmm_parsed.post_id), format!(r#"<div class="cmmauth">{}</div><div class="cmmtime">{}</div><div class="cmmtext">{}</div>"#, username, cmm_parsed.date, cmm_parsed.text)).map_err(|err| { format!("Redis err: {:?}", err) })?;

        Ok("OK".to_string())
    }

    fn get_list_of_posts(&self, username: &str) -> Result<String, String> {
        let last_key = "ilast_post";

        let mut curr_conn = match self.conn_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };

        let last_id: u32 = curr_conn.deref_mut().get(last_key).unwrap_or(0) + 1;
        let mut buffer_v: Vec<String> = vec![];
        buffer_v.reserve(last_id as usize);
        for x in 0..last_id {
            let title: String = curr_conn.deref_mut().get(&format!("title_{}", x)).unwrap_or("".to_string());
            if title.len() < 5 {
                continue;
            }
            buffer_v.push(format!(r#"<div class="linked_form">
                                        <form action="/dashboard/blogdev"  method="post" id="postpage_sender{}">
                                            <div class="command_f">
                                              <input type="hidden" name="qtype" value="R" class="qtype">
                                              <input type="hidden" name="group" value="blogdev_read" class="group">
                                              <input type="hidden" name="username" value="{}" class="username">
                                              <input type="hidden" name="command" value="getpost" class="command">
                                              <input type="hidden" name="payload" value="{}" class="payload">
                                            </div>
                                              <a onclick="document.getElementById('postpage_sender{}').submit();">{}</a>
                                        </form>
                            </div>"#, x, username, x, x, title));
        }
        Ok(format!(r#"<div class="post_list_block">
            <div class="posts_list_counter">{}</div>
            {}
        </div>
        <div class="ln_create_post">
        <form action="/dashboard/blogdev"  method="post" id="postpage_sender">
                <div class="command_f">
                  <input type="hidden" name="qtype" value="W" class="qtype">
                  <input type="hidden" name="group" value="blogdev_write" class="group">
                  <input type="hidden" name="username" value="{}" class="username">
                  <input type="hidden" name="command" value="showcreatepost" class="command">
                  <input type="hidden" name="payload" value="" class="payload">
                </div>
                  <a onclick="document.getElementById('postpage_sender').submit();">Create a post</a>
            </form>
        </div>
        "#, buffer_v.len(), buffer_v.join("\n"), &username))
    }
}


impl DeviceRead for BlogDevice {
    fn read_data(&self, query: &QCommand) -> Result<String, String> {
        let command = query.command.as_str();

        if query.group != "blogdev_read" {
            return Err("No access to this action".to_string());
        }

        match command {
            "getpost" => self.get_post(&query.username, &query.payload),
            _ => return Err(format!("Unknown for BlogDevice.read command: {}", command))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<String, String> {
        if query.group != "rstatus" {
            return Err("No access to this action".to_string());
        }
        self.get_list_of_posts(&query.username)
    }
}


impl DeviceWrite for BlogDevice {
    fn write_data(&self, query: &QCommand) -> Result<String, String> {
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
    fn request_query(&self, query: &QCommand) -> Result<String, String> {
        let command = query.command.as_str();

        if query.group != "blogdev_request" {
            return Err("No access to this action".to_string());
        }

        match command {
            "createcmm" => self.new_cmm(&query.username, &query.payload),
            _ => return Err(format!("Unknown for BlogDevice.read command: {}", command))
        }
    }
}

impl DeviceConfirm for BlogDevice {
    fn confirm_query(&self, _query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, _query: &QCommand) -> Result<String, String> {
        Err("Unimplemented".to_string())
    }
}