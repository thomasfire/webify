extern crate redis;
extern crate r2d2_redis;
extern crate scraper;
extern crate reqwest;

use scraper::{Html, Selector};
use redis::Commands;
use r2d2_redis::{RedisConnectionManager, r2d2};
use std::ops::DerefMut;
use std::thread;
use std::time::Duration;

use crate::database::get_hash;
use self::r2d2_redis::r2d2::PooledConnection;

type RedisPool = r2d2::Pool<RedisConnectionManager>;

const SHIKIMORI_URL: &str = "https://shikimori.one/forum/news";

fn keyen_hash(s_to_hash: &str) -> String {
    format!("article_{}", &get_hash(s_to_hash)[0..8])
}

fn get_header_links(curr_conn: &mut PooledConnection<RedisConnectionManager>) -> Result<Vec<String>, String> {
    let body = reqwest::blocking::get(SHIKIMORI_URL)
        .map_err(|err| { format!("Reqwest to shikimori failed: {:?}", err) })?.text()
        .map_err(|err| { format!("Reqwest to shikimori failed: {:?}", err) })?;

    let document = Html::parse_document(&body);
    let selector = Selector::parse("a.name[title]").map_err(|err| { format!("Failed to create select : {:?}", err) })?;

    let mut links: Vec<String> = vec![];
    links.reserve(10);

    for elem in document.select(&selector) {
        let hrlink = match elem.value().attr("href") {
            Some(val) => val,
            None => continue,
        };
        let hashed = keyen_hash(hrlink);
        let last_id: u32 = curr_conn.deref_mut().get(&hashed).unwrap_or(0);
        if last_id > 0 {
            continue;
        }
        links.push(hrlink.to_string());
        curr_conn.deref_mut().set(&hashed, 1).map_err(|err| { format!("Redis err: {:?}", err) })?;
    }

    Ok(links)
}

fn parse_and_write(curr_conn: &mut PooledConnection<RedisConnectionManager>, lnurl: &str) -> Result<(), String> {
    let body = reqwest::blocking::get(lnurl)
        .map_err(|err| { format!("Reqwest to shikimori failed at article: {:?}", err) })?.text()
        .map_err(|err| { format!("Reqwest to shikimori failed at article: {:?}", err) })?;

    let document = Html::parse_document(&body);
    let selector_header = Selector::parse("h1").map_err(|err| { format!("Failed to create select for header: {:?}", err) })?;
    let selector_article = Selector::parse("div.body-inner[itemprop=\"articleBody\"]").map_err(|err| { format!("Failed to create select for article: {:?}", err) })?;

    let titles = document.select(&selector_header)
        .map(|elem| { elem.inner_html() }).collect::<Vec<String>>();
    let title = titles.get(0).unwrap_or(&("".to_string())).clone();

    let articles = document.select(&selector_article)
        .map(|elem| { elem.inner_html() }).collect::<Vec<String>>();
    let article = articles.get(0).unwrap_or(&("".to_string())).clone();

    let last_key = "ilast_post";

    let last_id: u32 = curr_conn.deref_mut().get(last_key).unwrap_or(0);
    let curr_id = last_id + 1;
    curr_conn.deref_mut().set(&format!("title_{}", curr_id), &title).map_err(|err| { format!("Redis err: {:?}", err) })?;
    curr_conn.deref_mut().set(&format!("body_{}", curr_id), &article).map_err(|err| { format!("Redis err: {:?}", err) })?;
    curr_conn.deref_mut().set(&format!("cmmcount_{}", curr_id), 0).map_err(|err| { format!("Redis err: {:?}", err) })?;
    curr_conn.deref_mut().set("ilast_post", curr_id).map_err(|err| { format!("Redis err: {:?}", err) })?;

    Ok(())
}

fn perform_parsing(conn_pool: &RedisPool) -> Result<u32, String> {
    let mut curr_conn = match conn_pool.get() {
        Ok(val) => val,
        Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
    };

    let links = get_header_links(&mut curr_conn)?;
    let mut counter = 0;
    for x in &links {
        match parse_and_write(&mut curr_conn, x) {
            Ok(_) => counter+=1,
            Err(err) => { eprintln!("Error occured during parsing `{}`: {}", x, err); continue; }
        };
        thread::sleep(Duration::from_secs(2));
    }
    Ok(counter)
}

pub fn run_parsing(conn_pool: RedisPool) {
    println!("Starting parsing...");
    thread::spawn(move || {
        loop {
            println!("Result of parsing: {:?}", perform_parsing(&conn_pool));
            thread::sleep(Duration::from_secs(1 * 60 * 60));
        }
    });
}

