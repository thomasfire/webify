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
 * Add new groups for your device into the db via Root device (yeah, that's web panel)
 
 You can see the working example at `src/printer_device.rs`.
 
### Build and setup
You'll need the [Rust Lang](https://www.rust-lang.org/) compiler (at least 1.48).
You can easily install it on Manjaro:
```shell script
$ sudo pacman -S rust cargo openssl redis # for building only
$ sudo pacman -S rust cargo openssl rust-src rust-doc \
  rust-debugger-common rust-lldb rust-gdb rustfmt rust-packaging # for developing
```

As Webify works with SQLite as database, make sure you have installed the `libsqlite3-dev` 
on Debian or `sqlite-devel` on Fedora

Build on Linux is pretty simple:
```shell script
$ ./build.sh build  # for debug version
$ ./build.sh build --release  # for release version
```

After this you can copy the `target/[release, debug]/styles` and `target/[release, debug]/webify`
to place you want. Go to that place and run:
```shell script
$ ./webify --setup  # configure the database, address and printer
$ ./webify --uadd   # add your first user, remember that it must contain all the groups you need
$ openssl genrsa 4096 > key.pem  # generate key for TLS
$ openssl req -x509 -days 1000 -new -key key.pem -out cert.pem  # generate certificate for TLS
```

After this you can just run the server:
```shell script
$ ./webify
```

If you need additional documentation, you can run:
```shell script
$ cargo doc
```
And see it at `target/doc/webify/index.html`.