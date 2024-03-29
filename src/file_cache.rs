use log::trace;

use std::collections::BTreeMap;
use std::io::prelude::*;
use std::fs::File;
use std::sync::{RwLock, Arc};

#[derive(Clone, Default)]
pub struct FileCache {
    files: Arc<RwLock<BTreeMap<String, Vec<u8>>>>
}

impl FileCache {
    pub fn new() -> Self {
        FileCache { files: Arc::new(RwLock::new(BTreeMap::new())) }
    }
    pub fn reset(&mut self) {
        self.files.write().unwrap().clear();
    }
    pub fn get_byte_file(&mut self, filename: &str) -> Result<Vec<u8>, String> {
        if filename.contains("/") || filename.contains("..") {
            return Err(format!("Access to the {} is denied", filename));
        }
        let full_path = format!("static/{}", filename);
        match self.files.read().unwrap().get(&full_path) {
            Some(value) => return Ok(value.clone()),
            None => trace!("Loading `{}` into cache", filename)
        };
        let mut file = match File::open(&full_path) {
            Ok(f) => f,
            Err(e) => return Err(format!("Error on opening the file: {:?}", e))
        };

        let mut file_data: Vec<u8> = vec![];
        match file.read_to_end(&mut file_data) {
            Ok(_) => self.files.write().unwrap().insert(full_path, file_data.clone()),
            Err(err) => return Err(format!("Error on reading the file: {:?}", err))
        };
        Ok(file_data)
    }

    pub fn get_str_file(&mut self, filename: &str) -> Result<String, String> {
        match self.get_byte_file(filename) {
            Ok(value) => {
                match String::from_utf8(value) {
                    Ok(sval) => Ok(sval),
                    Err(err) => Err(format!("Error on converting the binary file into UTF8: {:?}", err))
                }
            }
            Err(err) => Err(format!("Error on getting the binary file: {:?}", err))
        }
    }
}