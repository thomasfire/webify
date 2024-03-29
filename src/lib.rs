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
pub mod devices;
pub mod device_trait;
pub mod root_device;
pub mod file_device;
pub mod printer_device;
pub mod file_cache;
pub mod blog_device;
pub mod stat_device;
pub mod stat_service;
pub mod autoban_service;
pub mod news_payload_parser;
pub mod shikimori_scraper;
pub mod template_cache;
pub mod ecg_device;