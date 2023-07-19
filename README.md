# dumb-cgi
An adequate, ~~essentially~~ dependencyless CGI library in Rust.

The purpose of this library is to allow server-side CGI programs to easily
parse requests (in particular "multipart/formdata" requests) and generate
responses without pulling in a bunch of full-featured crates. `dumb_cgi`
does not attempt (or at least won't start off attempting) to be
resource-efficient; its chief goals are simplicity and ease of use. Some
trade-offs it makes are:

  * It does a lot of copying and makes a lot of small allocations. This
    makes it easier to use (and write), but it carries a performance and
    resource usage penalty.
  
  * It forces lossy conversion of all environment variable and header names
    and values to UTF-8 (so they can be stored as `String`s). The spec
    should guarantee that header names are valid UTF-8, but if you need
    any of the other three (header values, environment variable names, or
    environment variable values) to be something that can't correctly,
    meaningfully be converted to UTF-8, then this crate is too dumb for
    your use case.
        
  * It doesn't make any effort to try to parse improperly-formed or
    almost-properly-formed requests, and it might even mishandle some
    uncommon corner cases in order to simplify implementation. For instance,
    the headers and blank lines in a multipart body part _must_ end with
    "\r\n" (which is strictly conformant to the spec), even though plenty
    of other HTTP implementations will still recognize plain ol' "\n"
    line endings. This simplifies the implementation.

  * Its intended use case is server-side CGI programs only. It supports
    _reading_ requests, but not making them, and _writing_ responses, but
    not reading them,  and only supports the parts of the HTTP-verse directly
    related to reading, parsing, and responding to CGI requests.

## Usage

To illustrate both reading and responding to requests, below is a sample
program that reads a request, logs the information about it, and then
returns a cursory "success" response. If any of the `.unwrap()`s or
`.expect()s` panic, the web server will just return a generic 500 response.

For logging, we will use macros from the
[`log`](https://crates.io/crates/log) logging facade and the functionality
of the
[`simplelog`](https://crates.io/crates/simplelog) logging crate (both of
which become dependencies if you compile `dumb-cgi` with the `log` feature).

```rust
use dumb_cgi::{Request, EmptyResponse, Query, Body};
use simplelog::{WriteLogger, LevelFilter, Config};

fn main() {
    // Open the log file.
    WriteLogger::init(
        LevelFilter::max(),
        Config::default(),
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("dumb_example.log")
            .unwrap()
    ).unwrap();
    
    // Gather info about the CGI request, including reading any body
    // (if present) from stdin.
    let request = Request::new().unwrap();
    
    // Log method request origin information.
    log::trace!(
        "Rec'd {} request from {} on port {}:",
        request.var("METHOD").unwrap_or("none"),
        request.var("REMOTE_ADDR").unwrap_or("nowhere"),
        request.var("SERVER_PORT").unwrap_or("-1")
    );
    
    // Log all the headers.
    //
    // The `Request::header()` method will return individual header values
    // (if present); the `Request::headers()` method will return an
    // iterator over all `(name, value)` header pairs.
    log::trace!("    Request headers:");
    for (name, value) in request.headers() {
        log::trace!("        {}: {}", name, value);
    }
    
    // If there's a query string, log info about it.
    //
    // The `Request::query()` method returns a reference to a
    // `dumb_cgi::Query` enum.
    match request.query() {
        Query::None => {
            log::trace!("    No query string.");
        },
        Query::Some(form) => {
            // If this variant is returned, then the query string was
            // parseable as `&`-separated `name=value` pairs, and the
            // contained `form` value is a `HashMap<String, String>`.
            log::trace!("    Form data from query string:");
            for (name, value) in form.iter() {
                log::trace!("        {}={}", name, value);
            }
        },
        Query::Err(e) => {
            // If this variant is returned, there was an error attempting
            // to parse the `QUERY_STRING` environment variable as a series
            // of `&`-separated `name=value` pairs.
            
            // `dumb_cgi::Error`s have a public `.details` member that is a
            // string with information about the error.
            log::trace!("    Error parsing query string: {}", &e.details);
            // You can still access the value of `QUERY_STRING` directly:
            log::trace!(
                "    Raw QUERY_STRING value: {}",
                request.var("QUERY_STRING").unwrap()
            );
        },
    }
    
    // If there's a body, log info about it.
    //
    // The `Request::body()` method returns a reference to a
    // `dumb_cgi::Body` enum.
    match request.body() {
        Body::None => {
            log::trace!("    No body.");
        },
        Body::Some(bytes) => {
            // Most valid bodies of properly-formed requests will return
            // this variant; `bytes` will be an `&[u8]`.
            log::trace!("    {} bytes of body.", bytes.len());
        },
        Body::Multipart(parts) => {
            // If the request has a properly-formed `Content-type` header
            // indicating `multipart/form-data`, and the body of the request
            // is also properly formed, this variant will be returned.
            //
            // The contained `parts` is a vector of `dumb_cgi::MultipartPart`
            // structs, one per part.
            log::trace!("    Multipart body with {} part(s).", parts.len());
        },
        Body::Err(e) => {
            // This variant will be returned if there is an error reading
            // the body.
            log::trace!("    Error reading body: {}", &e.details);
        },
    }
    
    // And we'll just put a blank line here in the log to separate
    // info about separate requests.
    log::trace!("");
    
    // Now that we've read and logged all the information we want from our
    // request, it's time to generate and send a response.
    //
    // Responses can be created with the builder pattern, starting with
    // an `EmptyResponse` (which has no body). In order to send a response
    // with a body, we need to call `EmptyResponse::with_content_type()`,
    // which turns our `EmptyResponse` into a `FullResponse`, which takes
    // a body.
    
    // Takes the HTTP response code.
    let response = EmptyResponse::new(200)
        // Headers can be added any time.
        .with_header("Cache-Control", "no-store")
        // Now we can add a body.
        .with_content_type("text/plain")
        // A body can be added this way; `FullResponse` also implements
        // `std::io::Write` for writing to the response body.
        .with_body("Success. Your request has been logged.")
        // Again, headers can be added any time.
        .with_header("Request-Status", "logged");
    
    // `FullResponse::respond()` consumes the response value and writes the
    // response to stdout.
    response.respond().unwrap();
}
```

Obviously, more details are available in the documentation.

## To Do

This may actually be more or less the final form of this crate (aside from bug
fixes or usability tweaks). I have ideas for improvements, but they all
drag this project dangerously into "less dumb" territory. Then again, I'm
not particularly fond of the idea of maintaining both `dumb_cgi` and
`not_quite_as_dumb_cgi`, so this may very well just feature creep and grow
warts until I'm disgusted with it.

## Notes

  * v 0.3.0: Removed dependence on
    [`lua-patterns`](https://crates.io/crates/lua-patterns),
    because even though I like the idea of it, and it has worked well for
    me in other projects, it kept panicing. `dumb_cgi` now depends only
    on the [`log`](https://crates.io/crates/log) logging facade (and
    [`simplelog`](https://crates.io/crates/simplelog) if you actually
    want to do some logging and enable the `log` feature).
    
  * v 0.4.0: Added explicit query string parsing.
  
  * v 0.5.0: Added response types and functionality.
    
  * v 0.6.0: Changed `Error` type and error handling.

  * v 0.7.0: [`log`](https://crates.io/crates/log) is now also an
    optional dependency.

  * v 0.8.0 Implemented `std::error::Error` for `dumb_cgi::Error`;
    fixed a typo in `README.md`.