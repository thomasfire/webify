use crate::config::Config;
use crate::stat_device;
use crate::template_cache::TemplateCache;
use crate::database::Database;

use log::{debug, info, error};
use r2d2_redis::{RedisConnectionManager, r2d2 as r2d2_red};
use redis::Commands;
use serde_json::{Value as jsVal, json};
use phf::{phf_map};

use std::thread;
use std::time::Duration;
use std::ops::DerefMut;

type RedisPool = r2d2_red::Pool<RedisConnectionManager>;

#[derive(Clone)]
struct Color {
    r: u16,
    g: u16,
    b: u16,
    alpha: f32,
}

const COLORS_SET: [Color; 10] = [
    Color { r: 255, g: 99, b: 132, alpha: 0.2 },
    Color { r: 255, g: 159, b: 64, alpha: 0.2 },
    Color { r: 255, g: 205, b: 86, alpha: 0.2 },
    Color { r: 75, g: 192, b: 192, alpha: 0.2 },
    Color { r: 54, g: 162, b: 235, alpha: 0.2 },
    Color { r: 237, g: 72, b: 114, alpha: 0.2 },
    Color { r: 153, g: 102, b: 255, alpha: 0.2 },
    Color { r: 201, g: 203, b: 207, alpha: 0.2 },
    Color { r: 94, g: 243, b: 207, alpha: 0.2 },
    Color { r: 243, g: 94, b: 231, alpha: 0.2 }
];


type Formatter1 = fn(u32) -> String;
type Formatter2 = fn(u32, &str) -> String;

fn chart_device_fmt(seconds: u32) -> String {
    format!("SELECT device as label, COUNT(*) as counter FROM history WHERE timestamp > date('now', '-{} second') GROUP BY device ORDER BY COUNT(timestamp) DESC LIMIT 10;", seconds)
}

fn chart_user_fmt(seconds: u32) -> String {
    format!("SELECT username as label, COUNT(*) as counter FROM history WHERE timestamp > date('now', '-{} second') GROUP BY username ORDER BY COUNT(timestamp) DESC LIMIT 10;", seconds)
}

fn chart_command_fmt(seconds: u32) -> String {
    format!("SELECT command as label, COUNT(*) as counter FROM history WHERE timestamp > date('now', '-{} second') GROUP BY command ORDER BY COUNT(timestamp) DESC LIMIT 10;", seconds)
}

fn chart_device_cross_user_fmt(seconds: u32, username: &str) -> String {
    format!("SELECT device as label, COUNT(*) as counter FROM history WHERE timestamp > date('now', '-{} second') AND username = '{}' GROUP BY device ORDER BY COUNT(timestamp) DESC LIMIT 10;", seconds, username)
}

fn chart_cmd_cross_user_fmt(seconds: u32, username: &str) -> String {
    format!("SELECT command as label, COUNT(*) as counter FROM history WHERE timestamp > date('now', '-{} second') AND username = '{}' GROUP BY command ORDER BY COUNT(timestamp) DESC LIMIT 10;", seconds, username)
}

static CHARTS_QUERIES: phf::Map<&'static str, Formatter1> = phf_map! {
    "chart_device" => chart_device_fmt,
    "chart_user" => chart_user_fmt,
    "chart_command" => chart_command_fmt
};

static CROSS_CHARTS_QUERIES: phf::Map<&'static str, Formatter2> = phf_map! {
    "chart_cmd_cross_user" => chart_cmd_cross_user_fmt,
    "chart_device_cross_user" => chart_device_cross_user_fmt
};

fn cache_general_stats(conn_pool: &RedisPool, database: &Database, template_cache: &TemplateCache, period_s: u32) -> Result<(), String> {
    let mut curr_conn = match conn_pool.get() {
        Ok(val) => val,
        Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
    };

    for chart in stat_device::STAT_CHARTS {
        let query: String = match CHARTS_QUERIES.get(chart) {
            Some(v) => v(period_s),
            None => {
                error!("No such chart available: `{}`", chart);
                continue;
            }
        };
        let stats_db = match database.load_stats_by_query(&query) {
            Ok(data) => data,
            Err(err) => {
                error!("Failed to load stats `{}`: `{}`", chart, err);
                continue;
            }
        };
        let mut labels_v: Vec<String> = vec![];
        labels_v.reserve(stats_db.len());
        let mut counter_v: Vec<i32> = vec![];
        counter_v.reserve(stats_db.len());

        let mut colors_simple_v: Vec<String> = vec![];
        colors_simple_v.reserve(stats_db.len());
        let mut colors_alpha_v: Vec<String> = vec![];
        colors_alpha_v.reserve(stats_db.len());

        for stat_entry in &stats_db {
            labels_v.push(stat_entry.label.clone());
            counter_v.push(stat_entry.counter);
        }
        for x in 0..stats_db.len() {
            let i = &COLORS_SET[x % COLORS_SET.len()];
            colors_simple_v.push(format!("rgb({}, {}, {})", i.r, i.g, i.b));
            colors_alpha_v.push(format!("rgb({}, {}, {}, {})", i.r, i.g, i.b, i.alpha));
        }
        let data: jsVal = json!({
            "simple_colors": colors_simple_v,
            "alpha_colors": colors_alpha_v,
            "data_values": counter_v,
            "labels": labels_v,
        });

        let render_res = match template_cache.render_template(&format!("{}.hjs", chart), &data) {
            Ok(v) => v,
            Err(err) => {
                error!("Failed to render stats `{}`: `{}`", chart, err);
                continue;
            }
        };

        match curr_conn.deref_mut().set::<&str, &str, ()>(chart, &render_res).map_err(|err| { format!("Redis err: {:?}", err) }) {
            Ok(_) => (),
            Err(err) => {
                error!("Failed to send stats to redis `{}`: `{}`", chart, err);
                continue;
            }
        };
    }
    Ok(())
}

fn cache_cross_stats(conn_pool: &RedisPool, database: &Database, template_cache: &TemplateCache, period_s: u32) -> Result<(), String> {
    let mut curr_conn = match conn_pool.get() {
        Ok(val) => val,
        Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
    };
    let users_db: Vec<String> = database.get_all_users()?.iter().map(|uentry| uentry.name.clone()).collect();

    for chart in stat_device::CROSS_STAT_CHARTS {
        for user in &users_db {
            let query: String = match CROSS_CHARTS_QUERIES.get(chart) {
                Some(v) => v(period_s, user),
                None => {
                    error!("No such chart available: `{}`", chart);
                    continue;
                }
            };
            let stats_db = match database.load_stats_by_query(&query) {
                Ok(data) => data,
                Err(err) => {
                    error!("Failed to load stats `{}`: `{}`", chart, err);
                    continue;
                }
            };
            let mut labels_v: Vec<String> = vec![];
            labels_v.reserve(stats_db.len());
            let mut counter_v: Vec<i32> = vec![];
            counter_v.reserve(stats_db.len());

            let mut colors_simple_v: Vec<String> = vec![];
            colors_simple_v.reserve(stats_db.len());
            let mut colors_alpha_v: Vec<String> = vec![];
            colors_alpha_v.reserve(stats_db.len());

            for stat_entry in &stats_db {
                labels_v.push(stat_entry.label.clone());
                counter_v.push(stat_entry.counter);
            }
            for x in 0..stats_db.len() {
                let i = &COLORS_SET[x % COLORS_SET.len()];
                colors_simple_v.push(format!("rgb({}, {}, {})", i.r, i.g, i.b));
                colors_alpha_v.push(format!("rgb({}, {}, {}, {})", i.r, i.g, i.b, i.alpha));
            }
            let data: jsVal = json!({
                "simple_colors": colors_simple_v,
                "alpha_colors": colors_alpha_v,
                "data_values": counter_v,
                "labels": labels_v,
            });

            let render_res = match template_cache.render_template(&format!("{}.hjs", chart), &data) {
                Ok(v) => v,
                Err(err) => {
                    error!("Failed to render stats `{}`: `{}`", chart, err);
                    continue;
                }
            };

            match curr_conn.deref_mut().set::<&str, &str, ()>(&format!("{}_{}", chart, user), &render_res)
                .map_err(|err| { format!("Redis err: {:?}", err) }) {
                Ok(_) => (),
                Err(err) => {
                    error!("Failed to send stats to redis `{}`: `{}`", chart, err);
                    continue;
                }
            };
        }
    }
    Ok(())
}

pub fn run_stat_service(conn_pool: &RedisPool, database: &Database, config: &Config) {
    debug!("Starting stat service...");
    let chart_period = config.general_stat_period_s;
    let cross_chart_period = config.cross_user_stat_period_s;
    let requested_period = config.period_to_request_s;
    if (chart_period == 0 && cross_chart_period == 0) || requested_period == 0 {
        return;
    }
    let template_cache = TemplateCache::new();
    debug!("Stat templates load result: {:?}", template_cache.load("templates/json"));

    if chart_period > 0 {
        let conn_pool_copy = conn_pool.clone();
        let database_copy = database.clone();
        let template_cache_copy = template_cache.clone();
        let request_period_copy = requested_period;
        thread::spawn(move || {
            loop {
                let res = cache_general_stats(&conn_pool_copy, &database_copy, &template_cache_copy, request_period_copy);
                info!("Cached general charts : {:?}", res);
                thread::sleep(Duration::from_secs(chart_period as u64));
            }
        });
    }
    if cross_chart_period > 0 {
        let conn_pool_copy = conn_pool.clone();
        let database_copy = database.clone();
        let template_cache_copy = template_cache.clone();
        let request_period_copy = requested_period;
        thread::spawn(move || {
            loop {
                let res = cache_cross_stats(&conn_pool_copy, &database_copy, &template_cache_copy, request_period_copy);
                info!("Cached cross user charts: {:?}", res);
                thread::sleep(Duration::from_secs(cross_chart_period as u64));
            }
        });
    }
}