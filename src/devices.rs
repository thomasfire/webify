use std::convert::TryFrom;

#[derive(Clone)]
pub enum Devices {
    Zero = 0,
    Filer = 1,
    Root = 2,
    Printer = 3,
    Blog = 4,
    Stat = 5,
    LEN,
}

pub enum Groups {
    RStatus = 0,
    Read = 1,
    Write = 2,
    Request = 3,
    Confirm = 4,
    Dismiss = 5,
    LEN,
}

pub const GROUP_LEN: usize = Groups::LEN as usize;
pub const DEVICES_LEN: usize = Devices::LEN as usize;

const DEVICE_ARRAY: [Devices; DEVICES_LEN] = [
    Devices::Zero,
    Devices::Filer,
    Devices::Root,
    Devices::Printer,
    Devices::Blog,
    Devices::Stat
];


pub const DEV_NAMES: [&'static str; DEVICES_LEN] = [
    "",
    "filer",
    "root",
    "printer",
    "blogdev",
    "statdev",
];

pub const DEV_GROUPS: [[Option<&'static str>; GROUP_LEN]; DEVICES_LEN] = [
    // [RStatus, Read, Write, Request, Confirm, Dismiss]
    [Some("rstatus"), None, None, None, None, None], // Zero device
    [None, Some("filer_read"), Some("filer_write"), None, None, None], // Filer device
    [None, Some("root_read"), Some("root_write"), None, None, None], // Root device
    [None, Some("printer_read"), Some("printer_write"), Some("printer_request"), Some("printer_confirm"), Some("printer_dismiss")], // Printer device
    [None, Some("blogdev_read"), Some("blogdev_write"), Some("blogdev_request"), None, None], // Blog device
    [None, Some("statdev_read"), None, None, None, None], // Stat device
];

pub fn list_all_groups() -> Vec<String> {
    let mut buffer: Vec<String> = vec![];
    for device in DEV_GROUPS {
        for group in device {
            match group {
                Some(s_group) => buffer.push(s_group.to_string()),
                None => continue
            };
        }
    }
    buffer
}

impl TryFrom<usize> for Devices {
    type Error = ();

    fn try_from(v: usize) -> Result<Self, Self::Error> {
        if v < DEVICES_LEN {
            Ok(DEVICE_ARRAY[v].clone())
        } else {
            Err(())
        }
    }
}