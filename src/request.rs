/*!
The `Request` type, which holds all the information about an HTTP request
available to a CGI program, together with all the attendant constants and
functions required to generate it.
*/

use std::collections::HashMap;
use std::io::Read;

use crate::Error;

const MULTIPART_CONTENT_TYPE: &str = "multipart/form-data";
const MULTIPART_BOUNDARY: &str = "boundary=";
const HTTP_NEWLINE: &[u8] = "\r\n".as_bytes();
/// Prefix used to identify whether an environment variable is actually
/// an HTTP header being passed on to the script.
const HTTP_PREFIX: &str = "HTTP_";

const PLUS: u8 = b'+';
const PERCENT: u8 = b'%';
const SPACE: u8 = b' ';

/*
Attempt to decode a %-encoded string (like in a CGI query string,
which is exactly what this function is used for).
*/
fn url_decode(qstr: &str) -> Result<String, String> {
    let bytes = qstr.as_bytes();
    let mut rbytes: Vec<u8> = Vec::with_capacity(qstr.len());
    let mut idx: usize = 0;

    while idx < bytes.len() {
        // This is safe because, per the preceding line, `idx` is guaranteed
        // to be less than the length of `bytes`.
        let &b = unsafe { bytes.get_unchecked(idx) };
        if b == PLUS {
            rbytes.push(SPACE);
            idx += 1;
        } else if b == PERCENT {
            let (start, end) = (idx + 1, idx + 3);
            match bytes.get(start..end) {
                Some(substr) => match std::str::from_utf8(substr) {
                    Ok(txt) => match u8::from_str_radix(txt, 16) {
                        Ok(n) => {
                            rbytes.push(n);
                            idx += 3;
                        }
                        Err(e) => {
                            let estr = format!("Error %-decoding at index {}: {}", idx, &e);
                            return Err(estr);
                        }
                    },
                    Err(e) => {
                        let estr = format!("Error %-decoding at index {}: {}", idx, &e);
                        return Err(estr);
                    }
                },
                None => {
                    let estr = "Query string ended during escape sequence.".to_owned();
                    return Err(estr);
                }
            }
        } else {
            rbytes.push(b);
            idx += 1;
        }
    }

    rbytes.shrink_to_fit();
    match String::from_utf8(rbytes) {
        Ok(s) => Ok(s),
        Err(e) => {
            let estr = format!("%-decoded query string not UTF-8: {}", &e);
            Err(estr)
        }
    }
}

/*
Return the offset of the beginning of `needle` in `haystack` (or `None`
if it's not there).

This is essentially analagous to the
[`str.find()`](https://doc.rust-lang.org/std/primitive.str.html#method.find)
method with another `str` as the argument, and really should be a standard
`slice` method.
*/
fn slicey_find<T: Eq>(haystack: &[T], needle: &[T]) -> Option<usize> {
    // slice::windows() panics if asked for windows of length 0,
    // so let's just return early and avoid that situation.
    if needle.is_empty() {
        return None;
    }

    for (n, w) in haystack.windows(needle.len()).enumerate() {
        if w == needle {
            return Some(n);
        }
    }

    None
}

/**
Struct holding a single part of a multipart/formdata body.

Like the request headers themselves, each part's headers have been lossily
converted to UTF-8. Names have been lower-cased and stripped of surrounding
whitespace. Values have had their _leading_ whitespace stripped, but any
trailing whitespace has been left intact.
*/
#[derive(Debug)]
pub struct MultipartPart {
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

/**
Type of body detected in the request.

This is not detected from the request method, but rather from the presence
(and values) of the `content-length` and `content-type` headers.
*/
#[derive(Debug)]
pub enum Body {
    /// The request has no `content-length` header.
    None,
    /// The request has a `content-length` header, but the `content-type`
    /// is something _other_ than `multipart/form-data.`
    Some(Vec<u8>),
    /// The request has a `content-length` header, and the `content-type`
    /// _is_ `multipart/form-data`. This will contain a vector of
    /// successfully-parsed body parts.
    Multipart(Vec<MultipartPart>),
    /// There was an error at some point in the process of determining the
    /// type of or reading/parsing the body.
    Err(Error),
}

/**
Type of query string detected in the request.

This is not detected from the request method, but rather the presence
and content of the `QUERY_STRING` environment variable.
*/
#[derive(Debug)]
pub enum Query {
    /// No `QUERY_STRING` environment variable.
    None,
    /// The `QUERY_STRING` environment variable's value was successfully
    /// split into `name=value` form data pairs and percent-decoded.
    Some(HashMap<String, String>),
    /// There was an error processing the value of the `QUERY_STRING`
    /// environment variable.
    ///
    /// This will happen if the query string isn't properly percent-encoded
    /// or formatted in `&`-separated `name=value` pairs. If this is the
    /// case, you can always get access to the raw value of the query
    /// string throught the `Request::var()` method.
    Err(Error),
}

/**
Struct holding details about your CGI environment and the request
that has been made to your program.
*/
#[derive(Debug)]
pub struct Request {
    vars: HashMap<String, String>,
    headers: HashMap<String, String>,
    query: Query,
    body: Body,
}

/**
An iterator over a `HashMap<String, String>` that yields
`(&'str, &'str)` tuples.

This is returned by the `Request::vars()` and `Request::headers()` methods,
for iterating over environment variables and request headers, respectively.
*/
pub struct Vars<'a>(std::collections::hash_map::Iter<'a, String, String>);

impl<'a> Iterator for Vars<'a> {
    type Item = (&'a str, &'a str);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/*
Given a slice of bytes, attempt to parse it as an HTTP header-style line
and return a `(name, value)` tuple.

Both `name` and `value` will be lossily converted to UTF-8. The `name` will
then have surrounding whitespace trimmed and be forced to lower-case; the
`value` will have _leading_ whitespace trimmed but otherwise left as-is.
*/
fn match_header(bytes: &[u8]) -> Option<(String, String)> {
    const COLON: u8 = b':';
    let sep_idx = match bytes.iter().position(|b| *b == COLON) {
        Some(n) => n,
        None => {
            return None;
        }
    };
    let key = String::from_utf8_lossy(&bytes[..sep_idx])
        .trim()
        .to_lowercase();
    let val = String::from_utf8_lossy(&bytes[(sep_idx + 1)..])
        .trim_start()
        .to_string();
    Some((key, val))
}

/*
Return the index of the next newline (after `current_position`) in `bytes`
that is immediately followed by `boundary`. This should be the first byte
after the end of the multipart/form-data body chunk that begins on or
after `current_position`.
*/
fn find_next_multipart_chunk_end(
    bytes: &[u8],
    current_position: usize,
    boundary: &[u8],
) -> Option<usize> {
    let mut pos = current_position;
    let mut subslice = &bytes[pos..];
    while let Some(n) = slicey_find(subslice, HTTP_NEWLINE) {
        let post_newline_idx = pos + n + HTTP_NEWLINE.len();
        if bytes.len() > post_newline_idx {
            subslice = &bytes[post_newline_idx..];
            if subslice.starts_with(boundary) {
                return Some(pos + n);
            }
            pos = post_newline_idx;
        }
    }
    None
}

/*
Takes a reference to a chunk of a multipart body that falls between two
boundaries, and returns that information in a `MultipartPart` struct.
*/
fn read_multipart_chunk(chunk: &[u8]) -> Result<MultipartPart, String> {
    let mut position: usize = 0;
    let mut headers: HashMap<String, String> = HashMap::new();

    while let Some(n) = slicey_find(&chunk[position..], HTTP_NEWLINE) {
        let next_pos = position + n;
        if let Some((k, v)) = match_header(&chunk[position..next_pos]) {
            headers.insert(k, v);
            position = next_pos + HTTP_NEWLINE.len();
        } else {
            position = next_pos + HTTP_NEWLINE.len();
            break;
        }
    }

    let body: Vec<u8> = chunk[position..].to_vec();

    Ok(MultipartPart { headers, body })
}

/*
Takes a reference to the body of a multipart/form-data request and
attempts to return a `Body::Multipart` variant.

This function (and the multipart body chunking code in particular) is
kind of a rats' nest of conditionals, so this function's interior
commentary errs on the side of excessiveness.
*/
fn read_multipart_body(body_bytes: &[u8], boundary: &str) -> Body {
    #[cfg(feature = "log")]
    {
        log::debug!(
            "read_multipart_body() called\n    boundary: \"{}\"",
            boundary
        );
        log::debug!("  {} body bytes", body_bytes.len());
    }

    let mut parts: Vec<MultipartPart> = Vec::new();

    // As per RFC 7578, the `boundary=...` value found in the `CONTENT_TYPE`
    // header will appear in the body with two hyphens prepended, so
    // `boundary_bytes` is prepared thus from the supplied header value.
    let prepended_boundary = {
        let mut b = String::with_capacity(boundary.len() + 2);
        b.push_str("--");
        b.push_str(boundary);
        b
    };
    let boundary_bytes = &prepended_boundary.as_bytes();

    // This will hold subslices of `body_bytes`, each of which will contain
    // the raw bytes of one "part" of the multipart body.
    let mut chunks: Vec<&[u8]> = Vec::new();

    /*
    Thus follows the multipart body chunking code. It grovels through the body
    of a multipart/form-data request (`body_bytes`), identifying the beginning
    and end of each part, and pushing the corresponding slice of bytes (a
    subslice of `body_bytes`) onto the `chunks` vector.
    */

    // First we set our initial position just after the first occurrence of
    // the boundary byte sequence.
    let mut position = match slicey_find(body_bytes, boundary_bytes) {
        Some(n) => {
            // If the boundary is found in the body, check to ensure there is
            // more body left after the end of the boundary (so we don't)
            // panic in our subsequent subslicing.
            let end_idx = n + boundary_bytes.len();
            let nl_end_idx = end_idx + HTTP_NEWLINE.len();
            if body_bytes.len() > nl_end_idx {
                // If there _is_ more body left after the boundary, check
                // whether the boundary is immediately followed by a newline.
                if &body_bytes[end_idx..nl_end_idx] == HTTP_NEWLINE {
                    // If so, set our starting position to be immediately
                    // after the newline.
                    nl_end_idx
                } else {
                    // If the boundary _isn't_ immediately followed by a
                    // newline, just return a `Body::Multipart` with an empty
                    // vector of parts.`
                    //
                    // *** Should this be an error instead?
                    return Body::Multipart(parts);
                }
            } else {
                // If there isn't any more body after the first occurrence of
                // the boundary, just return a `Body::Multipart` with an
                // empty vector of parts.
                //
                // *** Should this be an error instead?
                return Body::Multipart(parts);
            }
        }
        None => {
            // If the boundary isn't found in the body, return an error
            // indicating as much.
            let err = Error {
                code: 400,
                message: "Not a valid multipart/form-data body.".to_owned(),
                details: "multipart body missing boundary string".to_owned(),
            };
            return Body::Err(err);
        }
    };

    #[cfg(feature = "log")]
    log::debug!("  initial boundary position: {}", &position);

    // Now we find subesequent occurrences of a newline pattern immediately
    // followed by a boundary.
    while let Some(next_position) =
        find_next_multipart_chunk_end(body_bytes, position, boundary_bytes)
    {
        // Declare a chunk that goes from the previous `position` up to (but
        // not including) the newline, and push it onto the vector of chunks.
        let chunk = &body_bytes[position..next_position];
        chunks.push(chunk);

        // If the boundary is then immediately followed by another newline,
        // set the `position` (the beginning of the next chunk) to be
        // immediately after the newline.
        //
        // Otherwise, be finished finding chunks (the final boundary pattern)
        // should be immediately followed by "--".
        position = next_position + HTTP_NEWLINE.len() + boundary_bytes.len();
        let post_newline_pos = position + HTTP_NEWLINE.len();
        if body_bytes.len() >= post_newline_pos {
            if &body_bytes[position..post_newline_pos] == HTTP_NEWLINE {
                position = post_newline_pos;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    #[cfg(feature = "log")]
    log::debug!("  read {} multipart chunks", &chunks.len());

    /*
    Now all the chunks have been found, it's time to process each one into
    a `MultipartPart` struct which contains a map of headers and a vector
    of bytes for the individual parts' body.
    */
    for chunk in chunks.iter() {
        match read_multipart_chunk(chunk) {
            Err(_) => {
                // If there is an error with a given multipart chunk, it is
                // just ignored. There is not a simple way to indicate errors
                // in individual chunks to the consumer of this library.
            }
            Ok(mpp) => parts.push(mpp),
        }
    }

    Body::Multipart(parts)
}

/*
Huff from stdin and process if appropriate to return a `Body` enum.
*/
fn read_body(body_len: usize, content_type: Option<&str>) -> Body {
    let mut body_bytes: Vec<u8> = vec![0; body_len];
    let stdin = std::io::stdin();
    let mut stdin_lock = stdin.lock();
    if let Err(e) = stdin_lock.read_exact(&mut body_bytes) {
        let err = Error {
            code: 500,
            message: "Unable to read request body.".to_owned(),
            details: format!("Error reading request body: {}", &e),
        };
        return Body::Err(err);
    }

    if let Some(content_type) = content_type {
        if let Some(n) = content_type.find(MULTIPART_CONTENT_TYPE) {
            let next_idx = n + MULTIPART_CONTENT_TYPE.len();
            if let Some(n) = content_type[next_idx..].find(MULTIPART_BOUNDARY) {
                let next_idx = next_idx + n + MULTIPART_BOUNDARY.len();
                return read_multipart_body(&body_bytes, &content_type[next_idx..]);
            } else {
                let err = Error {
                    code: 400,
                    message:
                        "Content-type: multipart/form-data lacks valid boundary specification."
                            .to_owned(),
                    details: format!(
                        "Can't find boundary in Content-type header: {}",
                        content_type
                    ),
                };
                return Body::Err(err);
            }
        }
    }

    Body::Some(body_bytes)
}

/*
Attempt to return the form data that's been URL percent-encoded
and chunked into `&`-separated `name=value` pairs in the query
string.
*/
fn parse_query_string(qstr: &str) -> Query {
    let mut qmap: HashMap<String, String> = HashMap::new();

    for nvp in qstr.split('&') {
        match nvp.split_once('=') {
            Some((coded_name, coded_value)) => {
                let name = match url_decode(coded_name) {
                    Ok(s) => s,
                    Err(e) => {
                        let err = Error {
                            code: 400,
                            message: "Invalid query string.".to_owned(),
                            details: format!(
                                "Error decoding name in chunk \"{}={}\": {}",
                                coded_name, coded_value, &e
                            ),
                        };
                        return Query::Err(err);
                    }
                };
                let value = match url_decode(coded_value) {
                    Ok(s) => s,
                    Err(e) => {
                        let err = Error {
                            code: 400,
                            message: "Invalid query string.".to_owned(),
                            details: format!(
                                "Error decoding value in chunk \"{}={}\": {}",
                                coded_name, coded_value, &e
                            ),
                        };
                        return Query::Err(err);
                    }
                };

                qmap.insert(name, value);
            }
            None => {
                let err = Error {
                    code: 400,
                    message: "Invalid query string.".to_owned(),
                    details: format!("Chunk \"{}\" not a name=vlaue pair.", nvp),
                };
                return Query::Err(err);
            }
        }
    }

    Query::Some(qmap)
}

impl Request {
    pub fn new() -> Result<Request, Error> {
        #[cfg(feature = "log")]
        log::debug!("Request::new() called");

        let mut vars: HashMap<String, String> = HashMap::new();
        let mut headers: HashMap<String, String> = HashMap::new();

        for (k, v) in std::env::vars_os().map(|(os_k, os_v)| {
            let str_k = String::from(os_k.to_string_lossy());
            let str_v = String::from(os_v.to_string_lossy());
            (str_k, str_v)
        }) {
            if let Some(var_name) = k.strip_prefix(HTTP_PREFIX) {
                let lower_k = var_name.replace('_', "-").to_lowercase();
                #[cfg(feature = "log")]
                log::debug!("  \"{}\" -> \"{}\", value: \"{}\"", &k, &lower_k, &v);
                headers.insert(lower_k, v);
            } else {
                let upper_k = k.to_uppercase();
                #[cfg(feature = "log")]
                log::debug!("  \"{}\" -> \"{}\", value: \"{}\"", &k, &upper_k, &v);
                vars.insert(upper_k, v);
            }
        }

        let query = match vars.get("QUERY_STRING") {
            Some(qstr) => parse_query_string(qstr),
            None => Query::None,
        };

        let body = if let Some(len_str) = headers.get("content-length") {
            match len_str.parse::<usize>() {
                Err(e) => {
                    let err = Error {
                        code: 400,
                        message: "Invalid Content-length header value.".to_owned(),
                        details: format!(
                            "Error parsing Content-length header value \"{}\": {}",
                            len_str, &e
                        ),
                    };
                    Body::Err(err)
                }
                Ok(body_len) => {
                    read_body(body_len, headers.get("content-type").map(|x| x.as_str()))
                }
            }
        } else {
            Body::None
        };

        Ok(Request {
            vars,
            headers,
            query,
            body,
        })
    }

    /**
    Return the value of the environment variable `k` if it exists and has
    been exposed to the CGI program.

    `k` will be converted to `ALL_UPPERCASE` before the check is made.

    # Examples

    ```
    # use dumb_cgi::Request;
    let r = Request::new().unwrap();

    println!("{:?}", r.var("METHOD"));
    // Probably Some("GET") or Some("POST").
    ```
    */
    pub fn var<'a>(&'a self, k: &str) -> Option<&'a str> {
        let modded = k.to_uppercase();
        self.vars.get(&modded).map(|v| v.as_str())
    }

    /**
    Return an iterator over all of the `("VARIABLE", "value")` pairs of
    environment variables passed to the CGI program.
    */
    pub fn vars(&self) -> Vars {
        Vars(self.vars.iter())
    }

    /**
    Return the value corresponding to the header `k` if it exists and has
    been exposed to the CGI program.

    `k` will be converted to `quiet-kebab-case` before the comparison is
    made (all header names have been similarly mangled before being
    stored).

    # Examples

    ```
    # use dumb_cgi::Request;
    let r = Request::new().unwrap();

    println!("{:?}", r.var("content-type"));
    // None (if it's a GET request) or something like Some("text/json")
    // or Some("multipart/formdata").
    ````
    */
    pub fn header<'a>(&'a self, k: &str) -> Option<&'a str> {
        let modded = k.replace('_', "-").to_lowercase();
        self.headers.get(&modded).map(|v| v.as_str())
    }

    /**
    Return an iterator over all the `("header-name", "value")` pairs of
    the request headers that have been exposed to the CGI program.
    */
    pub fn headers(&self) -> Vars {
        Vars(self.headers.iter())
    }

    /**
    Return a reference to the request's decoded query string (if present).
    */
    pub fn query(&self) -> &Query {
        &self.query
    }

    /**
    Return a reference to the request's body.
    */
    pub fn body(&self) -> &Body {
        &self.body
    }
}
