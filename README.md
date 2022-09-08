# webify

### Webify every device

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
 * Add new groups for your device into `src/device.rs` `enum Devices`, `DEV_NAMES` and `DEV_GROUPS`
 
 You can see the working example at `src/printer_device.rs`.

### Why Webify?

1. It's quick - most requests are handled less than 10ms
2. It's modest - release binary is 10 MB after stripping
3. Small runtime - you can easily deploy it to your Raspberry Pi
4. Secure in mind - starting from idea
5. Strong user access rights - easily define, what each user can perform

### Build and setup
You'll need the [Rust Lang](https://www.rust-lang.org/) compiler (at least 1.62).
You can easily install it on Manjaro:
```shell script
$ sudo pacman -S rust cargo openssl redis sqlite # for building only
$ sudo pacman -S rust cargo openssl redis sqlite rust-src rust-doc \
  rust-debugger-common rust-lldb rust-gdb rustfmt rust-packaging # for developing
```

As Webify works with SQLite as database, make sure you have installed the `libsqlite3-dev` 
on Debian or `sqlite-devel` on Fedora, `sqlite` on Manjaro. You'll also need `redis` to have caching enabled.

Build on Linux is pretty simple:
```shell script
$ ./build.sh build  # for debug version
$ ./build.sh build --release  # for release version
```

After this you can copy the `target/[release, debug]/static` and `target/[release, debug]/webify`
to place you want. Go to that place and run:
```shell script
$ ./webify --setup  # configure the database, address and printer
$ ./webify --uadd   # add your first user, remember that it must contain all the groups you need
$ openssl genrsa 4096 > key.pem  # generate key for TLS
$ openssl req -x509 -days 1000 -new -key key.pem -out cert.pem  # generate certificate for TLS
```

Suggested groups for your first user: `rstatus,filer_read,filer_write,root_write,root_read,printer_read,printer_write,printer_request,printer_confirm,blogdev_write,blogdev_request,blogdev_read`.
Suggested groups for your basic users: `rstatus,filer_read,printer_read,printer_request,blogdev_request,blogdev_read`.

After this you can just run the server:
```shell script
$ ./run.sh
```

If you need additional documentation, you can run:
```shell script
$ cargo doc
```
And see it at `target/doc/webify/index.html`.