/*!
Types and methods for parsing/detecting/reporting your CGI environment,
headers, and body.
*/
use std::collections::{HashMap, hash_map};
use std::io::Read;
use lua_patterns::LuaPattern;

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
    pub subheaders: HashMap<String, Vec<u8>>,
    pub value: Vec<u8>,
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

impl Cgi {
    fn read_body(body_len: usize) -> Body {
        let mut body_bytes: Vec<u8> = vec![0; body_len];
        let stdin = std::io::stdin();
        let mut stdin_lock = stdin.lock();
        if let Err(e) = stdin_lock.read_exact(&mut body_bytes) {
            let estr = format!("Error reading request body: {}", &e);
            Body::Err(Error::InputError(estr))
        } else {
            Body::Some(body_bytes)
        }
    }
    
    pub fn new() -> Result<Cgi, Error> {
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
                    headers.insert(lower_k, String::from(v));
                },
                None => {
                    let upper_k = k.to_uppercase();
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
                Ok(body_len) => Cgi::read_body(body_len),
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