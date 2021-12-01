use crate::database::Database;
use crate::models::LineWebify;
use crate::dashboard::QCommand;
use crate::device_trait::*;

use serde_json::Value as jsVal;
use serde_json::json;
use serde_json::from_str as js_from_str;

#[derive(Clone)]
pub struct RootDev {
    database: Database,
}

impl RootDev {
    pub fn new(db: &Database) -> RootDev {
        RootDev { database: db.clone() }
    }

    fn read_users(&self) -> Result<jsVal, String> {
        let res = match self.database.get_all_users() {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };

        Ok(json!({
            "template": "root_users_table.hbs",
            "users": res.iter().map(|x| x.get_content()).collect::<jsVal>()
        }))
    }

    fn read_history(&self) -> Result<jsVal, String> {
        let res = match self.database.get_all_history() {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };
        Ok(json!({
            "template": "root_history_table.hbs",
            "entries": res.iter().map(|x| x.get_content()).collect::<jsVal>()
        }))
    }


    fn read_groups(&self) -> Result<jsVal, String> {
        let res = match self.database.get_all_groups() {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };

        Ok(json!({
            "template": "root_groups_table.hbs",
            "groups": res.iter().map(|(group, device)| {
                json!({"g_name": group, "devices": device})
            }).collect::<jsVal>()
        }))
    }

    fn insert_new_user(&self, query: &str) -> Result<String, String> {
        let data: jsVal = js_from_str(query).map_err(|err| { format!("Couldn't parse JSON: {:?}", err) })?;
        let name = match data.get("username") {
            Some(d) => d.as_str().unwrap_or(""),
            None => return Err(format!("Error on inserting users: invalid syntax: couldn't find username"))
        };

        let password = match data.get("password") {
            Some(d) => d.as_str().unwrap_or(""),
            None => return Err(format!("Error on inserting users: invalid syntax: couldn't find password"))
        };
        if password.len() < 1 || name.len() < 1 {
            return Err(format!("Username or password is in wrong format"));
        }

        let groups = data.get("groups").map(|data| { data.as_str() }).unwrap_or(None);
        match self.database.insert_user(name, password, groups) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on inserting user: {}", e))
        }
    }

    fn update_user_pass(&self, query: &str) -> Result<String, String> {
        let data: jsVal = js_from_str(query).map_err(|err| { format!("Couldn't parse JSON: {:?}", err) })?;
        let name = match data.get("username") {
            Some(d) => d.as_str().unwrap_or(""),
            None => return Err(format!("Error on updating users pass: invalid syntax: couldn't find username"))
        };

        let password = match data.get("password") {
            Some(d) => d.as_str().unwrap_or(""),
            None => return Err(format!("Error on updating users : invalid syntax: couldn't find password"))
        };

        if password.len() < 1 || name.len() < 1 {
            return Err(format!("Username or password is in wrong format"));
        }

        match self.database.update_user_pass(name, password) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on updating user pass: {}", e))
        }
    }

    fn update_user_group(&self, query: &str) -> Result<String, String> {
        let data: jsVal = js_from_str(query).map_err(|err| { format!("Couldn't parse JSON: {:?}", err) })?;
        let name = match data.get("username") {
            Some(d) => d.as_str().unwrap_or(""),
            None => return Err(format!("Error on updating users group: invalid syntax: couldn't find username"))
        };

        let groups = match data.get("groups") {
            Some(d) => d.as_str().unwrap_or(""),
            None => return Err(format!("Error on updating users group: invalid syntax: couldn't find groups"))
        };

        if groups.len() < 1 || name.len() < 1 {
            return Err(format!("Username or groups is in wrong format"));
        }

        match self.database.update_user_group(name, groups) {
            Ok(_) => Ok("Ok".to_string()),
            Err(e) => Err(format!("Error on updating user group: {}", e))
        }
    }
}


impl DeviceRead for RootDev {
    fn read_data(&self, query: &QCommand) -> Result<jsVal, String> {
        let command = query.command.as_str();
        if query.group != "root_read" {
            return Err("No access".to_string());
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
            return Err("No access".to_string());
        }

        match command {
            "add_user" => self.insert_new_user(query.payload.as_str()),
            "update_user_password" => self.update_user_pass(query.payload.as_str()),
            "update_user_groups" => self.update_user_group(query.payload.as_str()),
            _ => Err(format!("Unknown command"))
        }.map(|mess| {
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