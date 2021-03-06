/*!
CGI program for testing this library.
*/
use std::io::Write;

const FULL_BODY_LIMIT: usize = 64;
const BODY_PREV: usize = 8;

use dumb_cgi::{Body, EmptyResponse, FullResponse, Query, Request};
#[cfg(feature = "log")]
use simplelog::{Config, LevelFilter, WriteLogger};

#[derive(Debug)]
struct ErrorShim(String);

impl<D> From<D> for ErrorShim
where
    D: std::fmt::Display,
{
    fn from(d: D) -> Self {
        Self(format!("{}", &d))
    }
}

fn wrapped_main() -> Result<FullResponse, ErrorShim> {
    let cgi = match Request::new() {
        Ok(cgi) => cgi,
        Err(e) => {
            let estr = format!("Unable to parse environment: {:?}", &e.details);
            return Err(ErrorShim(estr));
        }
    };

    let mut r = EmptyResponse::new(200).with_content_type("text/plain");

    writeln!(&mut r, "Environment Variables:")?;
    for (k, v) in cgi.vars() {
        writeln!(&mut r, "    {}: {}", k, v)?;
    }

    writeln!(&mut r, "Exposed Headers:")?;
    for (k, v) in cgi.headers() {
        writeln!(&mut r, "    {}: {}", k, v)?;
    }

    writeln!(&mut r)?;
    match cgi.query() {
        Query::None => {
            writeln!(&mut r, "No query string.")?;
        }
        Query::Some(map) => {
            writeln!(&mut r, "Query analysis:")?;
            for (k, v) in map.iter() {
                writeln!(&mut r, "    {}: {}", k, v)?;
            }
        }
        Query::Err(e) => {
            writeln!(&mut r, "Error w/query string: {:?}", &e)?;
        }
    }

    writeln!(&mut r)?;
    match cgi.body() {
        Body::None => writeln!(&mut r, "No body."),
        Body::Some(b) => writeln!(&mut r, "{} bytes of body.", b.len()),
        Body::Multipart(v) => {
            writeln!(&mut r, "Multipart body with {} parts.", v.len())?;
            for (n, p) in v.iter().enumerate() {
                writeln!(&mut r, "  Part {}:", &n)?;
                for (k, v) in p.headers.iter() {
                    writeln!(&mut r, "    {}: {}", k, v)?;
                }
                writeln!(&mut r, "    {} bytes of body.", p.body.len())?;
                if p.body.len() > FULL_BODY_LIMIT {
                    let head = String::from_utf8_lossy(&(p.body)[..BODY_PREV]);
                    let tail = String::from_utf8_lossy(&(p.body)[(p.body.len() - BODY_PREV)..]);
                    writeln!(&mut r, "->|{} ... {}|<-", &head, &tail)?;
                } else {
                    let prev = String::from_utf8_lossy(&p.body);
                    writeln!(&mut r, "->|{}|<-", &prev)?;
                }
            }
            writeln!(&mut r)
        }
        Body::Err(e) => writeln!(&mut r, "Body: {:?}", &e),
    }?;

    Ok(r)
}

fn main() {
    #[cfg(feature = "log")]
    WriteLogger::init(
        LevelFilter::max(),
        Config::default(),
        std::fs::OpenOptions::new()
            .write(true)
            .open("/home/dan/testor.log")
            .unwrap(),
    )
    .unwrap();
    match wrapped_main() {
        Err(e) => {
            let err_body: Vec<u8> = e.0.into();
            let r = EmptyResponse::new(500)
                .with_content_type("text/plain")
                .with_body(err_body);
            r.respond().unwrap();
        }
        Ok(r) => r.respond().unwrap(),
    }
}
