/*!
Types and methods for parsing/detecting/reporting your CGI environment,
headers, and body.
*/
use std::collections::{HashMap, hash_map};
use std::io::Read;
use lua_patterns::LuaPattern;

const MULTIPART_CONTENT_TYPE: &str = "multipart/form-data";
const MULTIPART_BOUNDARY: &str = "boundary=";
const HTTP_NEWLINE: &str = "\r?\n";
const IMMEDIATE_NEWLINE: &str = "^\r?\n";
const MULTIPART_HEADER: &str = "([^:]-)%:(.-)\r?\n";

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
    if needle.len() == 0 { return None; }
    
    for (n, w) in haystack.windows(needle.len()).enumerate() {
        if w == needle { return Some(n) }
    }
    
    None
}

#[derive(Debug)]
pub enum Error {
    EnvironmentError(String),
    InputError(String),
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

#[derive(Debug)]
pub enum Body {
    None,
    Some(Vec<u8>),
    Multipart(Vec<MultipartPart>),
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
    body: Body,
}

/**
An iterator over a `HashMap<String, String>` that yields
`(&'str, &'str)` tuples.
*/
pub struct Vars<'a>(hash_map::Iter<'a, String, String>);

impl<'a> Iterator for Vars<'a> {
    type Item = (&'a str, &'a str);
    
    fn next(&mut self) -> Option<Self::Item> {
        match self.0.next() {
            None => None,
            Some((k, v)) => Some((k.as_str(), v.as_str())),
        }
    }
}

/*
Return the index of the next newline (after `current_position`) in `bytes`
that is immediately followed by `boundary`. This should be the first byte
after the end of the multipart/form-data body chunk that begins on or
after `current_position`.

`nl_patt` is a mutable reference to a `LuaPattern` built from "\r?\n",
which should match both strictly conforming HTTP metadata newlines
and also just regular Unicious newlines.
*/
fn find_next_multipart_chunk_end(
    bytes: &[u8],
    current_position: usize,
    boundary: &[u8],
    nl_patt: &mut LuaPattern
) -> Option<usize> {
    let mut pos = current_position;
    let mut subslice = &bytes[pos..];
    while nl_patt.matches_bytes(subslice) {
        let range = nl_patt.range();
        subslice = &bytes[(pos + range.end)..];
        if subslice.starts_with(boundary) {
            return Some(pos + range.start);
        } else {
            pos = pos + range.end;
        }
    }
    None
}

/*
Takes a reference to a chunk of a multipart body that falls between two
boundaries, and returns that information in a `MultipartPart` struct.
*/
fn read_multipart_chunk(chunk: &[u8]) -> Result<MultipartPart, String> {
    let mut patt = LuaPattern::new(MULTIPART_HEADER);
    let mut position: usize = 0;
    let mut headers: HashMap<String, String> = HashMap::new();
    
    while patt.matches_bytes(&chunk[position..]) {
        let k_range = patt.capture(1);
        let v_range = patt.capture(2);
        let (ks, ke) = (position + k_range.start, position + k_range.end);
        let (vs, ve) = (position + v_range.start, position + v_range.end);
        let k = String::from_utf8_lossy(&chunk[ks..ke]).trim().to_string();
        let v = String::from_utf8_lossy(&chunk[vs..ve])
                .trim_start().to_string();
        
        headers.insert(k, v);
        position = position + patt.range().end;
    }
    let post_header = &chunk[position..];
    if post_header.starts_with(b"\r\n") {
        position += 2;
    } else {
        position += 1;
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
fn read_multipart_body(
    body_bytes: &[u8],
    boundary: &str
) -> Body {
    log::debug!("read_multipart_body() called\n    boundary: \"{}\"", boundary);
    log::debug!("  {} body bytes", body_bytes.len());
    
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
    
    // The newline pattern will match either `LF` or `CRLF`, which makes this
    // more of a pain, but which also supports slightly non-conforming request
    // bodies. It is morally acceptable to support requests which fail to
    // conform in this particular way, because the whole "\r\n" thing is
    // obnoxious.
    let mut newline = LuaPattern::new(HTTP_NEWLINE);
    
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
            if body_bytes.len() > end_idx {
                // If there _is_ more body left after the boundary, check
                // whether the boundary is immediately followed by a newline
                // pattern.
                let mut imm_nl = LuaPattern::new(IMMEDIATE_NEWLINE);
                if imm_nl.matches_bytes(&body_bytes[end_idx..]) {
                    // If so, set our starting position to be immediately
                    // after the newline pattern.
                    end_idx + imm_nl.range().end
                } else {
                    // If the boundary _isn't_ immediately followed by a
                    // newline pattern, just return a `Body::Multipart`
                    // with an empty vector of parts.`
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
        },
        None => {
            // If the boundary isn't found in the body, return an error
            // indicating as much.
            let estr = "multipart body missing boundary string".to_string();
            return Body::Err(Error::InputError(estr));
        },
    };
    
    log::debug!("  initial boundary position: {}", &position);
    
    // Now we find subesequent occurrences of a newline pattern immediately
    // followed by a boundary.
    while let Some(next_position) = find_next_multipart_chunk_end(
        body_bytes, position, boundary_bytes, &mut newline
    ) {
        // Declare a chunk that goes from the previous `position` up to (but
        // not including) the newline pattern, and push it onto the vector
        // of chunks.
        let chunk = &body_bytes[position..next_position];
        chunks.push(chunk);
        
        // If the boundary is then immediately followed by another newline,
        // pattern set the `position` (the beginning of the next chunk) to
        // be immediately after the newline pattern.
        //
        // Otherwise, be finished finding chunks (the final boundary pattern)
        // should be immediately followed by "--".
        position = next_position + boundary_bytes.len();
        if newline.matches_bytes(&body_bytes[position..]) {
            position += newline.range().end;
        } else {
            break;
        }
    }
    
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
                // just ignored. There is not a simple way to indicate
                // errors in individual chunks to the consumer of this
                // library.
            },
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
        let estr = format!("Error reading request body: {}", &e);
        return Body::Err(Error::InputError(estr));
    }
    
    if let Some(content_type) = content_type {
        if let Some(n) = content_type.find(MULTIPART_CONTENT_TYPE) {
            let next_idx = n + MULTIPART_CONTENT_TYPE.len();
            if let Some(n) = content_type[next_idx..].find(MULTIPART_BOUNDARY) {
                let next_idx = next_idx + n + MULTIPART_BOUNDARY.len();
                return read_multipart_body(
                    &body_bytes,
                    &content_type[next_idx..]
                );
            } else {
                let err = Error::EnvironmentError(
                    "bad multipart boundary".to_string()
                );
                return Body::Err(err);
            }
        }
    }
    
    Body::Some(body_bytes)
}


impl Request {
    pub fn new() -> Result<Request, Error> {
        log::debug!("Request::new() called");
        let mut http_patt = LuaPattern::new("^HTTP_(.+)$");
        
        let mut vars: HashMap<String, String> = HashMap::new();
        let mut headers: HashMap<String, String> = HashMap::new();
        
        for (k, v) in std::env::vars_os().map(|(os_k, os_v)| {
            let str_k = String::from(os_k.to_string_lossy());
            let str_v = String::from(os_v.to_string_lossy());
            (str_k, str_v)
        }) {
            match http_patt.match_maybe(&k) {
                Some(var) => {
                    let lower_k = var.replace('_', "-").to_lowercase();
                    log::debug!("  \"{}\" -> \"{}\", value: \"{}\"", &k, &lower_k, &v);
                    headers.insert(lower_k, String::from(v));
                },
                None => {
                    let upper_k = k.to_uppercase();
                    log::debug!("  \"{}\" -> \"{}\", value: \"{}\"", &k, &upper_k, &v);
                    vars.insert(upper_k, String::from(v));
                }
            }
        }
        
        let body = if let Some(len_str) = headers.get("content-length") {
            match len_str.parse::<usize>() {
                Err(e) => {
                    let estr = format!(
                        "Error parsing CONTENT_LENGTH header: {}", &e
                    );
                    Body::Err(Error::EnvironmentError(estr))
                },
                Ok(body_len) => read_body(
                    body_len,
                    headers.get("content-type").map(|x| x.as_str())
                ),
            }
        } else {
            Body::None
        };
        
        Ok(Request { vars, headers, body })
    }

    /**
    Return the value of the environment variable `k` if it exists and has
    been exposed to the CGI program.
    
    `k` will be converted to `ALL_UPPERCASE` before the check is made.
    
    # Examples
    
    ```ignore
    let r = Request::new().unwrap();
    
    println!("{}", r.var("METHOD"));
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
    pub fn vars<'a>(&'a self) -> Vars<'a> {
        Vars(self.vars.iter())
    }
    
    /**
    Return the value corresponding to the header `k` if it exists and has
    been exposed to the CGI program.
    
    `k` will be converted to `quiet-kebab-case` before the comparison is
    made (all header names have been similarly mangled before being
    stored).
    
    # Examples
    
    ```ignore
    let r = Request::new().unwrap();
    
    println!("{}", r.var("content-type"));
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
    pub fn headers<'a>(&'a self) -> Vars<'a> {
        Vars(self.headers.iter())
    }
    
    /**
    Return a reference to the request's body.
    */
    pub fn body(&self) -> &Body { &self.body }
}