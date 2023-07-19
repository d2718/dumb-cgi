/*!
`dumb_cgi` is a library for collecting all the information made available by
the web server to a CGI program into a single, easy-to-use struct.

```rust
use std::io::Write;
use dumb_cgi::{Request, Query, Body, EmptyResponse};

// This example is wrapped in a main function that returns a
// `std::io::Result<()>` in order to leverage the `?` to cut down
// on some visual noise.

fn main() -> std::io::Result<()> {
    // Gather all request data from the environment and stdin.
    let req = Request::new().unwrap();

    // Instantiate a new response object, and give it a `Content-type` so that
    // the body can be written to.
    let mut response = EmptyResponse::new(200)
        .with_content_type("text/plain");

    // Write info about the environment to the response body.
    write!(&mut response, "Environment Variables:\n")?;
    for (var, value) in req.vars() {
        write!(&mut response, "    {}={}\n", var, value)?;
    }

    // Write info about the request headers to the response body.
    write!(&mut response, "\nExposed HTTP Headers:\n");
    for (header, value) in req.headers() {
        write!(&mut response, "    {}: {}\n", header, value);
    }

    // Write info about the parsed query string to the response body.
    match req.query() {
        Query::None => { write!(&mut response, "\nNo query string.")?; },
        Query::Some(map) => {
            write!(&mut response, "\nQuery String Form Data:\n")?;
            for (name, value) in map.iter() {
                write!(&mut response, "    {}={}\n", name, value)?;
            }
        },
        Query::Err(e) => {
            write!(&mut response, "\nError reading query string: {:?}\n", &e.details)?;
        },
    }

    // Write info about the request body to the response body.
    match req.body() {
        Body::None => { write!(&mut response, "\nNo body.\n")?; },
        Body::Some(bytes) => {
            write!(&mut response, "\n{} bytes of body.\n", bytes.len())?;
        },
        Body::Multipart(parts) => {
            write!(&mut response, "\nMultipart body with {} parts:\n", parts.len())?;
            for (n, part) in parts.iter().enumerate() {
                write!(&mut response, "    Part {}:\n", &n)?;
                for (header, value) in part.headers.iter() {
                    write!(&mut response, "        {}: {}\n", header, value)?;
                }
                write!(&mut response, "        {} bytes of body.\n", part.body.len())?;
            }
        },
        Body::Err(e) => {
            write!(&mut response, "\nError reading body: {:?}\n", &e.details)?;
        },
    }

    // Finally, send the response.
    response.respond()
}
```

The emphases are lack of dependencies and simplicity (both in use and in
maintenance). In pursuit of these, some trade-offs have been made.

  * Some CGI libraries use high-performance or fault-tolerant parsing
    techniques (like regular expressions). `dumb_cgi` is pretty
    straight-forward and doesn't go out of its way to deal with
    non- or almost-compliant requests, or even certain
    strictly-compliant cases that are unlikely and awkward to support.

  * Some libraries are fast and memory-efficient by avoiding allocation;
    they keep the body of the request around and only hand out references
    to parts of it. `dumb_cgi` copies and owns the parts of the request
    (and the environment) that it needs. This is simpler to both use
    and maintain.

  * `dumb_cgi` lossily converts everything except request bodies (and the
    "body" portions of `multipart/form-data` body parts) into UTF-8.
    This means not supporting certain strictly-compliant requests and
    possibly not correctly receiving environment variables on some systems,
    but is easier to both use and maintain. (If you _do_ need to deal
    with non-UTF-8 environment variables or header values, `dumb_cgi` is
    too dumb for your use case.)

  * `dumb_cgi`'s target is server-side CGI programs; it supports _reading_
    requests (not writing them), and _writing_ responses (not reading them).

# Features

`dumb_cgi` is dependency-free by default. Enabling the `log` feature
pulls in the [`log`](https://crates.io/crates/log) and
[`simplelog`](https://crates.io/crates/simplelog) crates, which are really
only for debugging `dumb_cgi` during its development. Consumers of this crate
shouldn't need this feature.

*/
use std::fmt::{Display, Formatter};

mod request;
pub use request::*;

mod response;
pub use response::*;

#[cfg(test)]
mod test;
/**
Errors returned by this libraray.

For convenience, each `Error` contains a suggested outward-facing message and
HTTP response code, but also some additional inward-facing details that
might help in debugging or troubleshooting.

Also, an `Error` can be turned directly into an HTTP response.

```rust
# use dumb_cgi::{Request, EmptyResponse, Error};
// Request::new() will return an `Error` if it can't read/parse all the
// necessary information supplied by the webserver about the request.

let response = match Request::new() {
    Ok(_) => EmptyResponse::new(200)
                .with_content_type("text/plain")
                .with_body("Your request was read successfully."),
    Err(e) => e.to_response(),
};

response.respond().unwrap();
```

*/
#[derive(Debug)]
pub struct Error {
    /// Recommended HTTP response code to use if sending an error response
    /// due to this error.
    pub code: u16,
    /// Recommended error message to return in an error response due to
    /// this error.
    pub message: String,
    /// Detailed description to return to the program for
    /// inspection/logging/etc.
    pub details: String,
}

impl Error {
    /**
    Consumes this error and returns an HTTP response appropriate to send
    back to the user agent.
    */
    pub fn to_response(self) -> FullResponse {
        EmptyResponse::new(self.code)
            .with_content_type("text/plain")
            .with_body(self.message)
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        write!(f, "{} ({})", &self.details, &self.code)
    }
}

impl std::error::Error for Error {}
