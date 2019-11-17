use diesel::prelude::*;
use chrono::NaiveDateTime;

use crate::schema::*;

#[derive(Queryable, PartialEq, Debug)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub password: String,
    pub cookie: Option<String>,
    pub groups: String,
    pub wrong_attempts: Option<u32>,
}


#[derive(Deserialize, Insertable)]
#[table_name = "users"]
pub struct UserAdd<'a> {
    pub name: &'a str,
    pub password: &'a str,
    pub groups: Option<&'a str>,
}


#[derive(Queryable, PartialEq, Debug)]
pub struct History {
    pub id: i32,
    pub get_query: String,
    pub timestamp: NaiveDateTime,
}

#[derive(Deserialize, Insertable)]
#[table_name = "history"]
pub struct HistoryForm<'a> {
    pub get_query: &'a str
}


#[derive(Queryable, PartialEq, Debug)]
pub struct Groups {
    pub id: i32,
    pub g_name: String,
    pub devices: String,
}


#[derive(Deserialize, Insertable)]
#[table_name = "groups"]
pub struct GroupAdd {
    pub g_name: String,
    pub devices: String,
}
