use crate::config::Config;
use crate::database::Database;
use crate::models::StatEntry;

use log::{debug, info, error};

use std::thread;
use std::time::Duration;

fn perform_autoban(database: &Database, period_to_view: u32, anomaly_f: f64) -> Result<(), String> {
    let stats: Vec<StatEntry> = database.load_stats_by_query(&format!("SELECT username, COUNT(*) as counter FROM history WHERE timestamp > date('now', '-{} second') GROUP BY username ORDER BY COUNT(timestamp) DESC;", period_to_view))?;
    let mut sum: usize = 0;
    for entry in &stats {
        sum += entry.counter as usize;
    }
    let avg = sum as f64 / stats.len() as f64;
    let mut to_ban: Vec<String> = vec![];
    for entry in &stats {
        if entry.counter as f64 > avg * anomaly_f {
            to_ban.push(entry.label.clone());
        }
    }

    debug!("Attempting to ban: `{}`", to_ban.join(", "));

    database.update_users_ban(&to_ban)
}

pub fn run_autoban_svc(database: &Database, config: &Config) {
    if config.autoban_period_s == 0 || config.autoban_anomaly_factor == 0.0 {
        return;
    }
    let db_copy = database.clone();
    let period = config.autoban_period_s;
    let period_view = config.period_to_request_s;
    let anomaly_f = config.autoban_anomaly_factor;

    thread::spawn(move || {
        loop {
            let res = perform_autoban(&db_copy, period_view, anomaly_f);
            match res {
                Ok(_) => info!("Autoban successfully made a round"),
                Err(err) => error!("Error occured in autoban: {}", err)
            };
            thread::sleep(Duration::from_secs(period as u64));
        }
    });
    info!("Autoban thread spawned");
}