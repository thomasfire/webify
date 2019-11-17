#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate diesel;

pub mod config;
pub mod io_tools;
pub mod server;
pub mod database;
pub mod dashboard;
pub mod models;
pub mod schema;