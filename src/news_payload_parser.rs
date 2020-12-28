extern crate regex;
extern crate chrono;

use chrono::Utc;

pub struct NewsPostParsed {
    pub title: String,
    pub body: String,
}

pub struct NewsCmmParsed {
    pub post_id: u32,
    pub text: String,
    pub date: String,
}

pub fn parse_post(payload: &str) -> Result<NewsPostParsed, String> {
    let title_reg = match regex::Regex::new(r"<title>(.*?)</title>") {
        Ok(val) => val,
        Err(err) => return Err(format!("Error on creating title regexp: {}", err))
    };
    let body_reg = match regex::Regex::new(r"<body>(.*?)</body>") {
        Ok(val) => val,
        Err(err) => return Err(format!("Error on creating body regexp: {}", err))
    };

    let title_s = match title_reg.find(payload) {
        Some(val) => val.as_str().to_string(),
        None => return Err(format!("Invalid title"))
    };
    let body_s = match body_reg.find(payload) {
        Some(val) => val.as_str().to_string(),
        None => return Err(format!("Invalid body"))
    };

    Ok(NewsPostParsed { title: title_s, body: body_s })
}


pub fn parse_cmm(payload: &str) -> Result<NewsCmmParsed, String> {
    let id_reg = match regex::Regex::new(r"<id>(.*?)</id>") {
        Ok(val) => val,
        Err(err) => return Err(format!("Error on creating id regexp: {}", err))
    };
    let text_reg = match regex::Regex::new(r"<text>(.*?)</text>") {
        Ok(val) => val,
        Err(err) => return Err(format!("Error on creating body regexp: {}", err))
    };

    let id_s = match id_reg.find(payload) {
        Some(val) => val.as_str().parse::<u32>().map_err(|err| { format!("Error on parsing cmm, no id: {:?}", err) })?,
        None => return Err(format!("Invalid title"))
    };
    let text_s = match text_reg.find(payload) {
        Some(val) => val.as_str().to_string(),
        None => return Err(format!("Invalid body"))
    };

    Ok(NewsCmmParsed { post_id: id_s, text: text_s, date: format!("{}", Utc::now()) })
}