use std::fs::File;
use std::io;
use std::io::prelude::*;
use std::io::Read;
use std::path::Path;

/// Reads filename and returns String
///
/// Reads filename and returns String, with replaced CRLF to LF
///
/// # Examples
///
/// ```rust
/// let file_string = read_str("path/to/file");
/// ```
pub fn read_str(filename: &str) -> Result<String, String> {
    let mut f = File::open(Path::new(filename)).unwrap();
    let mut buffer = String::new();
    match f.read_to_string(&mut buffer) {
        Ok(_) => print!(""),
        Err(err) => {
            eprintln!("Error on reading the file: {:?}", err);
            return Err("Error".to_string());
        }
    };
    Ok(String::from(buffer.replace("\r\n", "\n").trim()))
}

/// Reads line from stdin and returns trimed String
///
/// Reads line from stdin and returns trimed String.
/// It is similar to `input()` in Python, which returns striped string
///
/// # Examples
/// ```rust
/// let some_useful_string = read_std_line();
/// ```
pub fn read_std_line(output: &str) -> String {
    let mut buffer = String::new();
    print!("{}", output);
    io::stdout().flush().unwrap();
    io::stdin()
        .read_line(&mut buffer)
        .expect("Couldn`t read std");
    String::from(buffer.trim())
}

/// Returns true if path exists and false if not
pub fn exists(path: &str) -> bool {
    Path::new(path).exists()
}

/// Writes String to your file
///
/// # Examples
///
/// ```rust
/// write_to_file("/path/to/file", "I`m file");
/// ```
pub fn write_to_file(path: &str, content: String) -> Result<(), io::Error> {
    let mut file = File::create(Path::new(path)).unwrap();
    file.write_fmt(format_args!("{}", content))
}


/// Writes Vec<u8> to your file
///
/// # Examples
///
/// ```rust
/// write_to_file("/path/to/file", vec![82, 82, 62]);
/// ```
pub fn write_bytes_to_file(path: &str, content: Vec<u8>) -> Result<usize, io::Error> {
    let mut file = File::create(Path::new(path)).unwrap();
    file.write(&content)
}

