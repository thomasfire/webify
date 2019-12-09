use crate::dashboard::QCommand;
use crate::device_trait::*;
use crate::config::Config;

#[derive(Clone, Serialize, Deserialize)]
struct PrConfig {
    printer: String,
    storage: String,
}

#[derive(Clone)]
pub struct PrinterDevice {
    //db_conn: Pool, // TODO
    config: PrConfig
}
