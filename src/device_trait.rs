use crate::dashboard::QCommand;


pub trait DeviceRead {
    fn read_data(&self, query: &QCommand) -> Result<String, String>;
    fn read_status(&self, query: &QCommand) -> Result<String, String>;
}


pub trait DeviceWrite {
    fn write_data(&self, query: &QCommand) -> Result<String, String>;
}


pub trait DeviceRequest {
    fn request_query(&self, query: &QCommand) -> Result<String, String>;
}


pub trait DeviceConfirm {
    fn confirm_query(&self, query: &QCommand) -> Result<String, String>;
    fn dismiss_query(&self, query: &QCommand) -> Result<String, String>;
}

