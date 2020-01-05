use crate::dashboard::QCommand;

/// See examples in `src/printer_device.rs`, `src/file_device.rs` and `src/root_device.rs`

/// Manages read commands for specific device
pub trait DeviceRead {
    fn read_data(&self, query: &QCommand) -> Result<String, String>;
    fn read_status(&self, query: &QCommand) -> Result<String, String>;
}

/// Manages write commands for specific device
pub trait DeviceWrite {
    fn write_data(&self, query: &QCommand) -> Result<String, String>;
}

/// Manages request commands for specific device
pub trait DeviceRequest {
    fn request_query(&self, query: &QCommand) -> Result<String, String>;
}

/// Manages confirm and dismiss commands for specific device
pub trait DeviceConfirm {
    fn confirm_query(&self, query: &QCommand) -> Result<String, String>;
    fn dismiss_query(&self, query: &QCommand) -> Result<String, String>;
}

