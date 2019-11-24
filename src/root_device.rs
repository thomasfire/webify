use crate::device_trait::*;
use diesel::{r2d2, SqliteConnection};
use diesel::r2d2::ConnectionManager;

use crate::database::{get_all_users, get_all_history, get_all_groups, insert_user, update_user_pass, update_user_group, insert_group, update_group};
use crate::models::LineWebify;

type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

pub struct RootDev {
    db_pool: Pool
}

impl RootDev {
    pub fn new(db: Pool) -> RootDev {
        RootDev { db_pool: db }
    }

    fn read_users(&self) -> Result<String, String> {
        let res = match get_all_users(&self.db_pool) {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };

        if res.len() == 0 {
            return Ok("".to_string());
        } else {
            return Ok(format!("<table class=\"utable\">
            <tr>
            <th>id</th>
            <th>username</th>
            <th>password</th>
            <th>cookie</th>
            <th>groups</th>
            <th>wrong attempts</th>
            </tr>
            {}
            </table>", res.iter().map(|x| x.get_content())
                .collect::<Vec<String>>()
                .join("\n")));
        }
    }

    fn read_history(&self) -> Result<String, String> {
        let res = match get_all_history(&self.db_pool) {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };

        if res.len() == 0 {
            return Ok("".to_string());
        } else {
            return Ok(format!("<table class=\"htable\">
            <tr>
            <th>id</th>
            <th>username</th>
            <th>query</th>
            <th>timestamp</th>
            </tr>
            {}
            </table>", res[..100].iter().map(|x| x.get_content())
                .collect::<Vec<String>>()
                .join("\n")));
        }
    }


    fn read_groups(&self) -> Result<String, String> {
        let res = match get_all_groups(&self.db_pool) {
            Ok(d) => d,
            Err(err) => return Err(format!("Error in RootDev.read_users: {}", err))
        };

        if res.len() == 0 {
            return Ok("".to_string());
        } else {
            return Ok(format!("<table class=\"gtable\">
            <tr>
            <th>id</th>
            <th>group name</th>
            <th>devices</th>
            </tr>
            {}
            </table>", res.iter().map(|x| x.get_content())
                .collect::<Vec<String>>()
                .join("\n")));
        }
    }

    fn insert_new_user(&self, query: &str) -> Result<String, String> {
        let data: Vec<String> = query.split("::").map(|x| x.to_string()).collect();
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
        let data: Vec<String> = query.split("::").map(|x| x.to_string()).collect();
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
        let data: Vec<String> = query.split("::").map(|x| x.to_string()).collect();
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
        let data: Vec<String> = query.split("::").map(|x| x.to_string()).collect();
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
        let data: Vec<String> = query.split("::").map(|x| x.to_string()).collect();
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
    fn read_data(&self, query: &str) -> Result<String, String> {
        if query.len() < 8 {
            return Err("Command is too short".to_string());
        }
        let command = &query[0..8];
        match command {
            "ureadall" => self.read_users(),
            "hreadall" => self.read_groups(),
            "greadall" => self.read_history(),
            _ => Err(format!("Unknown command"))
        }
    }

    fn read_status(&self) -> Result<String, String> {
        Ok(format!("Root is ready to use.\nNow you can view and write data."))
    }
}

impl DeviceWrite for RootDev {
    fn write_data(&mut self, query: &str) -> Result<String, String> {
        if query.len() <= 9 {
            return Err("Command is too short".to_string());
        }
        let command = &query[0..9];
        match command {
            "uwritenew" => self.insert_new_user(&query[9..]),
            "uupdpsswd" => self.update_user_pass(&query[9..]),
            "uupdgroup" => self.update_user_group(&query[9..]),
            "gwritenew" => self.insert_new_group(&query[9..]),
            "gupdgroup" => self.update_group(&query[9..]),
            _ => Err(format!("Unknown command"))
        }
    }
}