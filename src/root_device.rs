use crate::database::{get_all_users, get_all_history, get_all_groups, insert_user, update_user_pass, update_user_group, insert_group, update_group};
use crate::models::LineWebify;
use crate::dashboard::QCommand;
use crate::device_trait::*;

use diesel::{r2d2, SqliteConnection};
use diesel::r2d2::ConnectionManager;
use serde_json::Value as jsVal;
use serde_json::json;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[derive(Clone)]
pub struct RootDev {
    db_pool: Pool
}

impl RootDev {
    pub fn new(db: &Pool) -> RootDev {
        RootDev { db_pool: db.clone() }
    }

    fn read_users(&self) -> Result<jsVal, String> {
        let res = match get_all_users(&self.db_pool) {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };

        Ok(json!({
            "template": "root_users_table.hbs",
            "users": res.iter().map(|x| x.get_content()).collect::<jsVal>()
        }))
    }

    fn read_history(&self) -> Result<jsVal, String> {
        let res = match get_all_history(&self.db_pool) {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };
        Ok(json!({
            "template": "root_history_table.hbs",
            "users": res.iter().map(|x| x.get_content()).collect::<jsVal>()
        }))
    }


    fn read_groups(&self) -> Result<jsVal, String> {
        let res = match get_all_groups(&self.db_pool) {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };

        Ok(json!({
            "template": "root_groups_table.hbs",
            "users": res.iter().map(|x| x.get_content()).collect::<jsVal>()
        }))
    }

    fn insert_new_user(&self, query: &str) -> Result<String, String> {
        let data: Vec<String> = query.split("---").map(|x| x.to_string()).collect();
        let name = match data.get(0) {
            Some(d) => d,
            None => return Err(format!("Error on inserting users: invalid syntax: couldn't find username"))
        };

        let password = match data.get(1) {
            Some(d) => d,
            None => return Err(format!("Error on inserting users: invalid syntax: couldn't find password"))
        };

        let groups = data.get(2);
        match insert_user(&self.db_pool, name, password, groups) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on inserting user: {}", e))
        }
    }

    fn update_user_pass(&self, query: &str) -> Result<String, String> {
        let data: Vec<String> = query.split("---").map(|x| x.to_string()).collect();
        let name = match data.get(0) {
            Some(d) => d,
            None => return Err(format!("Error on updating users pass: invalid syntax: couldn't find username"))
        };

        let password = match data.get(1) {
            Some(d) => d,
            None => return Err(format!("Error on updating users : invalid syntax: couldn't find password"))
        };

        match update_user_pass(&self.db_pool, name, password) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on updating user pass: {}", e))
        }
    }

    fn update_user_group(&self, query: &str) -> Result<String, String> {
        let data: Vec<String> = query.split("---").map(|x| x.to_string()).collect();
        let name = match data.get(0) {
            Some(d) => d,
            None => return Err(format!("Error on updating users group: invalid syntax: couldn't find username"))
        };

        let groups = match data.get(1) {
            Some(d) => d,
            None => return Err(format!("Error on updating users group: invalid syntax: couldn't find groups"))
        };

        match update_user_group(&self.db_pool, name, groups) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on updating user group: {}", e))
        }
    }

    fn insert_new_group(&self, query: &str) -> Result<String, String> {
        let data: Vec<String> = query.split("---").map(|x| x.to_string()).collect();
        let group = match data.get(0) {
            Some(d) => d,
            None => return Err(format!("Error on insert_new_group: invalid syntax: couldn't find group"))
        };

        let devices = match data.get(1) {
            Some(d) => d,
            None => return Err(format!("Error on insert_new_group: invalid syntax: couldn't find devices"))
        };

        match insert_group(&self.db_pool, group, devices) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on insert_new_group: {}", e))
        }
    }


    fn update_group(&self, query: &str) -> Result<String, String> {
        let data: Vec<String> = query.split("---").map(|x| x.to_string()).collect();
        let group = match data.get(0) {
            Some(d) => d,
            None => return Err(format!("Error on update_group: invalid syntax: couldn't find group"))
        };

        let devices = match data.get(1) {
            Some(d) => d,
            None => return Err(format!("Error on update_group: invalid syntax: couldn't find devices"))
        };

        match update_group(&self.db_pool, group, devices) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on update_group: {}", e))
        }
    }
}


impl DeviceRead for RootDev {
    fn read_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();
        if query.group != "root_read" {
            return Err("No access".to_string())
        }
        match command {
            "read_all_users" => self.read_users(),
            "read_all_hist" => self.read_history(),
            "read_all_groups" => self.read_groups(),
            _ => Err(format!("Unknown command"))
        }
    }

    fn read_status(&self, query: &QCommand) -> Result<jsVal, String> {
        Ok(json!({
            "template": "root_status.hbs",
            "username": query.username}))
    }
}

impl DeviceWrite for RootDev {
    fn write_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();
        if query.group != "root_write" {
            return Err("No access".to_string())
        }

        match command {
            "add_user" => self.insert_new_user(query.payload.as_str()),
            "update_user_password" => self.update_user_pass(query.payload.as_str()),
            "update_user_groups" => self.update_user_group(query.payload.as_str()),
            "add_group" => self.insert_new_group(query.payload.as_str()),
            "update_group" => self.update_group(query.payload.as_str()),
            _ => Err(format!("Unknown command"))
        }.map(|mess|{
             json!({
                "template": "simple_message.hbs",
                "message": mess})
         })
    }
}


impl DeviceRequest for RootDev {
    fn request_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}

impl DeviceConfirm for RootDev {
    fn confirm_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }

    fn dismiss_query(&self, _query: &QCommand) -> Result<jsVal, String> {
        Err("Unimplemented".to_string())
    }
}