/*!
CGI program for testing this library.
*/
use std::fmt::Write;

use dumb_cgi::{Body, Cgi};

#[derive(Debug)]
struct ErrorShim(String);

impl<D> From<D> for ErrorShim
where D: std::fmt::Display,
{
    fn from(d: D) -> Self { Self(format!("{}", &d)) }
}

fn wrapped_main() -> Result<String, ErrorShim> {
    let cgi = match Cgi::new() {
        Ok(cgi) => cgi,
        Err(e) => {
            let estr = format!("Unable to parse environment: {:?}", e);
            return Err(ErrorShim(estr));
        },
    };
    
    let mut r = String::new();
    write!(&mut r, "Environment Variables:\n")?;
    for (k, v) in cgi.vars() {
        write!(&mut r, "    {}: {}\n", k, v)?;
    }
    
    write!(&mut r, "Exposed Headers:\n")?;
    for (k, v) in cgi.headers() {
        write!(&mut r, "    {}: {}\n", k, v)?;
    }
    
    write!(&mut r, "\n")?;
    match cgi.body() {
        Body::None => write!(&mut r, "No body.\n"),
        Body::Some(b) => write!(&mut r, "{} bytes of body.\n", b.len()),
        Body::Multipart(v) => write!(
            &mut r,
            "Multipart body with {} parts.\n",
            v.len()
        ),
        Body::Err(e) => write!(&mut r, "Body: {:?}\n", &e),
    }?;
    
    Ok(r)
}

fn main() {
    match wrapped_main() {
        Err(e) => {
            let estr = format!("{:?}", &e);
            print!("Content-type: text/plain\r\n");
            print!("Content-length: {}\r\n", &estr.len());
            print!("\r\n");
            print!("{}", &estr);
        },
        Ok(out) => {
            print!("Content-type: text/plain\r\n");
            print!("Content-length: {}\r\n", &out.len());
            print!("\r\n");
            print!("{}", &out);
        }
    }
}
