use chrono::NaiveDateTime;
use serde_json::Value as jsVal;
use serde_json::json;

use crate::schema::*;

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
    pub groups: String
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
    pub username: Option<String>,
    pub get_query: String,
    pub timestamp: NaiveDateTime,
}

impl LineWebify for History {
    fn get_content(&self) -> jsVal {
        json!({
            "id": self.id,
            "username": self.username.as_ref().unwrap_or(&"".to_string()),
            "get_query": self.get_query,
            "timestamp": self.timestamp.format("%Y-%m-%d %H:%M:%S").to_string()
        })
    }
}


#[derive(Deserialize, Insertable)]
#[table_name = "history"]
pub struct HistoryForm<'a> {
    pub get_query: &'a str
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
