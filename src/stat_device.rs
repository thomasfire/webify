use crate::database::Database;
use crate::device_trait::*;
use crate::dashboard::QCommand;
use crate::config::Config;
use crate::devices::{Devices, Groups, DEV_GROUPS};
use crate::stat_service::run_stat_service;
use crate::autoban_service::run_autoban_svc;

use serde_json::{Value as jsVal, json};
use redis::Commands;
use r2d2_redis::{RedisConnectionManager, r2d2 as r2d2_red};

use std::ops::DerefMut;

type RedisPool = r2d2_red::Pool<RedisConnectionManager>;

#[derive(Clone)]
pub struct StatDevice {
    redis_pool: RedisPool,
    database: Database,
}

pub const STAT_CHARTS: [&'static str; 3] = [
    "chart_device",
    "chart_user",
    "chart_command"
];

pub const CROSS_STAT_CHARTS: [&'static str; 2] = [
    "chart_cmd_cross_user",
    "chart_device_cross_user",
];

impl StatDevice {
    pub fn new(database: &Database, config: &Config) -> Self {
        let manager = RedisConnectionManager::new(config.redis_cache.as_str()).unwrap();
        let pool = RedisPool::builder().build(manager).unwrap();
        run_stat_service(&pool, database, config);
        run_autoban_svc(database, config);
        StatDevice { redis_pool: pool, database: database.clone() }
    }

    fn get_chart_data(&self, username: &str, chart_name: &str) -> Result<jsVal, String> {
        let mut curr_conn = match self.redis_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };

        let chart_data: String = curr_conn.deref_mut().get(chart_name)
            .map_err(|err| format!("Error on getting the chart {}: {:?}", chart_name, err))?;

        Ok(json!({
            "template": "stat_chart.hbs",
            "chart_data": chart_data,
            "chart_name": chart_name,
            "rootw_access": self.database.has_access_to_group(username, DEV_GROUPS[Devices::Root as usize][Groups::Write as usize].unwrap()) // for ban-hammer
        }))
    }

    fn get_cross_chart_data(&self, username: &str, chart_name: &str, payload: &str) -> Result<jsVal, String> {
        let mut curr_conn = match self.redis_pool.get() {
            Ok(val) => val,
            Err(err) => return Err(format!("Error on getting current redis conn: {:?}", err))
        };

        let chart_data: String = curr_conn.deref_mut().get(format!("{}_{}", chart_name, payload))
            .map_err(|err| format!("Error on getting the chart {}: {:?}", chart_name, err))?;

        Ok(json!({
            "template": "stat_cross_chart.hbs",
            "chart_data": chart_data,
            "chart_name": chart_name,
            "chart_user": payload,
            "rootw_access": self.database.has_access_to_group(username, DEV_GROUPS[Devices::Root as usize][Groups::Write as usize].unwrap()) // for ban-hammer
        }))
    }
}

impl DeviceRead for StatDevice {
    fn read_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();

        if query.group != DEV_GROUPS[Devices::Stat as usize][Groups::Read as usize].unwrap() {
            return Err("No access to this action".to_string());
        }

        if STAT_CHARTS.contains(&command) {
            self.get_chart_data(&query.username, command)
        } else if CROSS_STAT_CHARTS.contains(&command) {
            self.get_cross_chart_data(&query.username, command, &query.payload)
        } else {
            Err(format!("Unknown for StatDevice.read command: {}", command))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<jsVal, String> {
        if query.group != DEV_GROUPS[Devices::Zero as usize][Groups::RStatus as usize].unwrap() {
            return Err("No access to this action".to_string());
        }
        Ok(json!({
            "template": "stat_status.hbs",
            "username": query.username
        }))
    }
}


impl DeviceWrite for StatDevice {
    fn write_data(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceRequest for StatDevice {
    fn request_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceConfirm for StatDevice {
    fn confirm_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}
