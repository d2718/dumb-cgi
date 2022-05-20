use crate::{Body, EmptyResponse, Query, Request};
use simplelog::{Config, LevelFilter, WriteLogger};

#[test]
fn readme_main() {
    // Open the log file.
    WriteLogger::init(
        LevelFilter::max(),
        Config::default(),
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("dumb_example.log")
            .unwrap(),
    )
    .unwrap();

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
    // `dumb_cgi::Body` enum.
    match request.query() {
        Query::None => {
            log::trace!("    No query string.");
        }
        Query::Some(form) => {
            // If this variant is returned, then the query string was
            // parseable as `&`-separated `name=value` pairs, and the
            // contained `form` value is a `HashMap<String, String>`.
            log::trace!("    Form data from query string:");
            for (name, value) in form.iter() {
                log::trace!("        {}={}", name, value);
            }
        }
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
        }
    }

    // If there's a body, log info about it.
    //
    // The `Request::body()` method returns a reference to a
    // `dumb_cgi::Body` enum.
    match request.body() {
        Body::None => {
            log::trace!("    No body.");
        }
        Body::Some(bytes) => {
            // Most valid bodies of properly-formed requests will return
            // this variant; `bytes` will be an `&[u8]`.
            log::trace!("    {} bytes of body.", bytes.len());
        }
        Body::Multipart(parts) => {
            // If the request has a properly-formed `Content-type` header
            // indicating `multipart/form-data`, and the body of the request
            // is also properly formed, this variant will be returned.
            //
            // The contained `parts` is a vector of `dumb_cgi::MultipartPart`
            // structs, one per part.
            log::trace!("    Multipart body with {} part(s).", parts.len());
        }
        Body::Err(e) => {
            // This variant will be returned if there is an error reading
            // the body.
            log::trace!("    Error reading body: {}", &e.details);
        }
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
