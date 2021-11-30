use chrono::NaiveDateTime;
use serde_json::Value as jsVal;
use serde_json::json;

use crate::schema::*;

#[derive(Copy, Clone)]
pub enum RejectReason {
    Ok = 0,
    NoAuth = 1,
    Error = 2,
}
const REJECTED_REASON: &'static [&'static str] = &[
    "OK",
    "NOT AUTHORIZED",
    "ERROR"
];

/// Represents that structure can be inserted in the table
pub trait LineWebify {
    fn get_content(&self) -> jsVal;
}

/// Represents user structure.
/// User has id, username, password (should be always hashified),  cookie (if does have),
/// groups user have, and number of wrong attempts he has made.
#[derive(Deserialize, Serialize, Queryable, PartialEq, Debug, Identifiable)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub password: String,
    pub groups: String,
}

impl LineWebify for User {
    fn get_content(&self) -> jsVal {
        json!({
            "id": self.id,
            "name": self.name,
            "password": self.password,
            "groups": self.groups
        })
    }
}

#[derive(Deserialize, Insertable)]
#[table_name = "users"]
pub struct UserAdd<'a> {
    pub name: &'a str,
    pub password: &'a str,
    pub groups: &'a str,
}


#[derive(Queryable, PartialEq, Debug)]
pub struct History {
    pub id: i32,
    pub username: String,
    pub device: String,
    pub command: String,
    pub qtype: String,
    pub rejected: i32,
    pub timestamp: NaiveDateTime,
}

impl LineWebify for History {
    fn get_content(&self) -> jsVal {
        json!({
            "id": self.id,
            "username": self.username,
            "device": self.device,
            "command": self.command,
            "qtype": self.qtype,
            "rejected": if self.rejected >= 0 && (self.rejected as usize) < REJECTED_REASON.len() {
                format!("{} {}", REJECTED_REASON[self.rejected as usize], self.rejected)
            } else {
                format!("UNKNOWN {}", self.rejected)
            } ,
            "timestamp": self.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
        })
    }
}


#[derive(Deserialize, Insertable)]
#[table_name = "history"]
pub struct HistoryForm<'a> {
    pub username: &'a str,
    pub device: &'a str,
    pub command: &'a str,
    pub qtype: &'a str,
    pub rejected: i32,
}

/// Groups is the structure, that matches group name with the device it has
#[derive(Queryable, PartialEq, Debug)]
pub struct Groups {
    pub id: i32,
    pub g_name: String,
    pub devices: String,
}


impl LineWebify for Groups {
    fn get_content(&self) -> jsVal {
        json!({
            "id": self.id,
            "g_name": self.g_name,
            "devices": self.devices
        })
    }
}


#[derive(Deserialize, Insertable)]
#[table_name = "groups"]
pub struct GroupAdd<'a> {
    pub g_name: &'a str,
    pub devices: &'a str,
}
