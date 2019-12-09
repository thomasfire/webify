# webify
Webify every device

Build your own device, that will be available via Web-Interface. 
Here is your `Build a web_device check-list`:
 * create a file for your device: `src/web_device.rs`
 * add its name to the `src/lib.rs`: 
 ```rust
pub mod web_device;
```
 * Implement Device Traits, defined in the `src/device_trait.rs`
 * Add your device name into the `src/dashboard.rs` into the function `Dispatch::resolve_by_name` 
 and add initializer into `Dispatch::new`
 * Add new web services in the `src/server.rs`, if you want to receive specific data (not necessary)
 * Add new groups for your device into the db via Root device (yeah, that's web panel)
 
