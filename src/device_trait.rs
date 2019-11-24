pub trait DeviceRead {
    fn read_data(&self, query: &str) -> Result<String, String>;
    fn read_status(&self) -> Result<String, String>;
}


pub trait DeviceWrite {
    fn write_data(&mut self, query: &str) -> Result<String, String>;
}


pub trait DeviceRequest {
    fn request_query(&mut self, query: &str) -> Result<String, String>;
}


pub trait DeviceConfirm {
    fn confirm_query(&mut self, query: &str) -> Result<String, String>;
    fn dismiss_query(&mut self, query: &str) -> Result<String, String>;
}

