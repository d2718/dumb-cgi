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
const MULTIPART_HEADER: &str = "^%s*([^:]-)%s*:%s*(.-)\r?\n";

#[derive(Debug)]
pub enum Error {
    EnvironmentError(String),
    InputError(String),
}

/**
Struct holding a single part of a multipart/formdata body.
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
pub struct Cgi {
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
Takes a reference to the boundary string that follows the
`; boundary=...` in the `content-type` header for formdata/multi-part
and returns a string for building a `LuaPattern` that will match
the boundary in the request body.
*/
fn patternize_string(boundary: &str) -> String {
    let mut s = String::with_capacity(boundary.len() * 2);

    // Add the extra two hyphens at the beginning.
    s.push('%'); s.push('-');
    s.push('%'); s.push('-');
    
    for c in boundary.chars() {
        match c {
            '-' | '(' | ')' | '.' | '%' | '+' | '*' | '[' | '^' | '$' => {
                s.push('%');
            },
            _ => { /* don't do anything */ },
        }
        s.push(c);
    }
    
    s
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
        let k = String::from_utf8_lossy(&chunk[ks..ke]).to_string();
        let v = String::from_utf8_lossy(&chunk[vs..ve]).to_string();
        
        headers.insert(k, v);
        position = position + patt.range().end;
    }
    
    let body: Vec<u8> = chunk[position..].to_vec();
    
    Ok(MultipartPart { headers, body })
}

/*
Takes a reference to the body of a multipart/form-data request and
attempts to return a `Body::Multipart` variant.
*/
fn read_multipart_body(
    body_bytes: &[u8],
    boundary: &str
) -> Body {
    log::debug!("read_multipart_body() called\n    boundary: \"{}\"", boundary);
    log::debug!("  {} body bytes", body_bytes.len());
    
    let mut parts: Vec<MultipartPart> = Vec::new();
    let mut patt_string = patternize_string(boundary);
    log::debug!("  pattern string is \"{}\"", &patt_string);
    let mut boundary = LuaPattern::new(&patt_string);
    let mut newline = LuaPattern::new(HTTP_NEWLINE);
    
    let mut chunks: Vec<&[u8]> = Vec::new();
    
    // Set the start of the first chunk after the first appearance
    // of the boundary sequence.
    let mut position = if boundary.matches_bytes(body_bytes) {
        let pos = boundary.range().end;
        if newline.matches_bytes(&body_bytes[pos..]) {
            let r = newline.range();
            
            // If there isn't a newline directly after the boundary pattern,
            // just return zero parts.
            if r.start != 0 {
                return Body::Multipart(parts);
            } else {
                pos + r.end
            }
        } else {
            // If there aren't any more newlines after the boundary pattern,
            // just go ahead and return zero parts.
            return Body::Multipart(parts);
        }
    } else {
        let estr = "boundary string not found in multipart body".to_string();
        return Body::Err(Error::InputError(estr));
    };
    
    log::debug!("  initial boundary position: {}", &position);
    
    // After the initial boundary, all occurrences of the boundary pattern
    // _should_ have newlines directly before them that _aren't_ part of the
    // previous part.
    patt_string.insert_str(0, HTTP_NEWLINE);
    log::debug!("  subsequent patternized string: {:?}", &patt_string);
    let mut boundary = LuaPattern::new(&patt_string);
    
    // Seek subsequent appearances of the boundary pattern and separate
    // the body into chunks.
    while boundary.matches_bytes(&body_bytes[position..]) {
        let boundary_range = boundary.range();
        let boundary_start = position + boundary_range.start;
        let boundary_end = position + boundary_range.end;
        log::debug!("    next boundary range: {}..{}", &boundary_start, &boundary_end);
        log::debug!("    chunk range: {}..{}", &position, &boundary_start);
        chunks.push(&body_bytes[position..boundary_start]);
        position = boundary_end;
    }
    log::debug!("  read {} multipart chunks", &chunks.len());
    
    // Process the chunks by separating them into headers and data.
    for chunk in chunks.iter() {
        match read_multipart_chunk(chunk) {
            Err(_) => { /* ignore this error */ },
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


impl Cgi {
    pub fn new() -> Result<Cgi, Error> {
        log::debug!("Cgi::new() called");
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
        
        Ok(Cgi { vars, headers, body })
    }

    /**
    Return the value of the environment variable `k` if it exists and has
    been exposed to the CGI program.
    
    `k` will be converted to `ALL_UPPERCASE` before the check is made.
    
    # Examples
    
    ```ignore
    let cgi = Cgi::new().unwrap();
    
    println!("{}", cgi.var("METHOD"));
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
    let cgi = Cgi::new().unwrap();
    
    println!("{}", cgi.var("content-type"));
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