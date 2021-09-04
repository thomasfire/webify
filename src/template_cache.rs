use handlebars::Handlebars;
use serde::Serialize;


use std::sync::{Arc, RwLock};
use std::fs::{File, read_dir};
use std::io::Read;

#[derive(Clone, Default)]
pub struct TemplateCache<'a> {
    templater: Arc<RwLock<Handlebars<'a>>>,
}

impl TemplateCache<'_> {
    pub fn new() -> Self {
        TemplateCache { templater: Arc::new(RwLock::new(Handlebars::new())) }
    }

    pub fn load(&self, path_to_add: &str) -> Result<(), String> {
        let entries = read_dir(path_to_add).map_err(|err| { format!("Error on reading templates directory {}: {:?}", path_to_add, err) })?;
        let mut err_counter: u16 = 0;
        {
            let mut handler = self.templater.write().unwrap();
            for entry in entries {
                if entry.is_err() { continue; }
                let entry_path = match entry {
                    Ok(data) => data,
                    Err(err) => {
                        eprintln!("Couldn't read file: {:?}", err);
                        err_counter += 1;
                        continue;
                    }
                };
                if !entry_path.path().is_file() { continue; }
                let mut str_buf = String::new();
                match File::open(&entry_path.path()) {
                    Ok(mut data) => match data.read_to_string(&mut str_buf) {
                        Ok(_) => (),
                        Err(err) => {
                            eprintln!("Error on reading file: {:?}", err);
                            err_counter += 1;
                            continue;
                        }
                    },
                    Err(err) => {
                        eprintln!("Couldn't open file: {:?}", err);
                        continue;
                    }
                };
                let name = entry_path.file_name().to_string_lossy().to_string();
                match handler.register_template_string(&name, str_buf).map_err(|err| {
                    eprintln!("Error in registering the template {}: {:?}", name, err);
                    err_counter += 1;
                }) {
                    Ok(_) => continue,
                    Err(_) => continue,
                };
            }
        }
        if err_counter > 0 {
            Err(format!("{} errors occured! Check logs for details or contact server administrator.", err_counter))
        } else {
            Ok(())
        }
    }

    pub fn render_template<T>(&self, tmpl: &str, data: &T) -> Result<String, String> where T: Serialize{
        self.templater.read().unwrap().render(tmpl, data).map_err(|err| {
            format!("Error in rendering {}: {:?}", tmpl, err)
        })
    }
}