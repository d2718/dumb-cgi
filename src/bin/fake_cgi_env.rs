/*!
Fakes a CGI environment for testing.
*/
use std::io::Write;
use std::process::{Command, Stdio};

const EXEC: &str = "target/debug/testor";

const BOUNDARY: &str = "--asdfjkl;0987654321";

const TEXT_VALS: &[(&str, &str)] = &[
    ("frogs", "ribbig"),
    ("macroexpand", "A cool but frustrating piece of tech."),
];

const FILES: &[&str] = &["/home/dan/home_ip.txt"];

fn main() {
    let mut buff: Vec<u8> = Vec::new();

    for (name, val) in TEXT_VALS.iter() {
        write!(&mut buff, "--{}\r\n", BOUNDARY).unwrap();
        write!(&mut buff, "Content-disposition: form-data; ").unwrap();
        write!(&mut buff, "name = \"{}\"\r\n", name).unwrap();
        write!(&mut buff, "\r\n").unwrap();
        write!(&mut buff, "{}\r\n", val).unwrap();
    }

    for file in FILES.iter() {
        let data = std::fs::read_to_string(file).unwrap();
        write!(&mut buff, "--{}\r\n", BOUNDARY).unwrap();
        write!(&mut buff, "Content-disposition: form-data; ").unwrap();
        write!(&mut buff, "name=\"{}\"; filename=\"{}\"\r\n", file, file).unwrap();
        write!(&mut buff, "\r\n").unwrap();
        write!(&mut buff, "{}\r\n", &data).unwrap();
    }

    write!(&mut buff, "--{}--\r\n", BOUNDARY).unwrap();

    let mut content_type = String::from("multipart/form-data; boundary=");
    content_type.push_str(BOUNDARY);
    let content_length = format!("{}", buff.len());

    let mut proc = Command::new(EXEC)
        .stdin(Stdio::piped())
        .env("RUST_BACKTRACE", "1")
        .env("HTTP_CONTENT_TYPE", &content_type)
        .env("HTTP_CONTENT_LENGTH", &content_length)
        .env("METHOD", "POST")
        .env("PROTOCOL", "HTTP/1.1")
        .spawn()
        .unwrap();

    let mut stdin = proc.stdin.take().unwrap();
    stdin.write_all(&buff).unwrap();
    proc.wait().unwrap();
}
