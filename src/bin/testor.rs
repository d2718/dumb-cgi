/*!
CGI program for testing this library.
*/
use std::fmt::Write;

const FULL_BODY_LIMIT: usize = 64;
const BODY_PREV: usize = 8;

#[cfg(feature = "log")]
use simplelog::{WriteLogger, LevelFilter, Config};
use dumb_cgi::{Body, Request};

#[derive(Debug)]
struct ErrorShim(String);

impl<D> From<D> for ErrorShim
where D: std::fmt::Display,
{
    fn from(d: D) -> Self { Self(format!("{}", &d)) }
}

fn wrapped_main() -> Result<String, ErrorShim> {
    let cgi = match Request::new() {
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
        Body::Multipart(v) => {
            write!(
                &mut r,
                "Multipart body with {} parts.\n",
                v.len()
            )?;
            for (n, p) in v.iter().enumerate() {
                write!(&mut r, "\n  Part {}:\n", &n)?;
                for (k, v) in p.headers.iter() {
                    write!(&mut r, "    {}: {}\n", k, v)?;
                }
                write!(&mut r, "    {} bytes of body.\n", p.body.len())?;
                if p.body.len() > FULL_BODY_LIMIT {
                    let head = String::from_utf8_lossy(&(p.body)[..BODY_PREV]);
                    let tail = String::from_utf8_lossy(&(p.body)[(p.body.len()-BODY_PREV)..]);
                    write!(&mut r, "->|{} ... {}|<-\n", &head, &tail)?;
                } else {
                    let prev = String::from_utf8_lossy(&p.body);
                    write!(&mut r, "->|{}|<-\n", &prev)?;
                }
                
            }
            write!(&mut r, "\n")
        },
        Body::Err(e) => write!(&mut r, "Body: {:?}\n", &e),
    }?;
    
    Ok(r)
}

fn main() {
    #[cfg(feature = "log")]
    WriteLogger::init(
        LevelFilter::max(),
        Config::default(),
        std::fs::OpenOptions::new().write(true)
            .open("/home/dan/testor.log").unwrap()
    ).unwrap();
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
