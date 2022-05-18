/*!
The two response types, `EmptyResponse` and `FullResponse`, to help build
(and deliver) responses to CGI requests.
*/

use std::collections::{HashMap, hash_map::Entry};
use std::io::Write;

/*
Internal value used to store `Response` header name-value pairs.

When a header is added to a `Response` (with one of several methods), a
lower-cased version of the passed name is used as a hash key, and the
unchanged version of the name is stored in a `HeaderValue`, along with the
value, of course. This is mainly to prevent the user from specifying
an incorrect value for `Content-length` (it can be easily overwritten
in the call to `.respond()` because the key is guaranteed to be the
all-lower-case `"content-length"`). It also prevents multiple header
specifications whose names differ by capitalization, which is maybe
not _wrong_, but is also probably not intentional.

According to [RFC2616](https://www.w3.org/Protocols/rfc2616/rfc2616-sec4.html#sec4.2),
headers with duplicate names _are_ allowed; however,

```text
Header-name: value0
Header-name: value1
```

is equivalent to

```text
Header-name: value0, value1
```

So multiple insertions of the same header will map the former form to
the latter form.

This is also important, because according to that same RFC, the order
in which these multiple values occur might matter. Because `dumb_cgi`
sends response headers by iterating through a `HashMap` (which does
_not_ guarantee any ordering), this rearrangement also guarantees
multiple values appear in the same order they are added.
*/
#[derive(Debug, Clone)]
struct HeaderValue {
    name: String,
    value: String,
}

/**
A response with no body.

Both `EmptyResponse::new()` and `FullResponse::new()` create values of this
type. Until the `.with_content_type()` method is called (consuming its
receiver and returning a `FullResponse`), a response may not have a body
added or bytes written to its body.
*/
#[derive(Debug)]
pub struct EmptyResponse {
    status: u16,
    headers: HashMap<String, HeaderValue>,
}

impl EmptyResponse {
    /**
    Create a new, headerless, empty response with the given HTTP status code.
    
    Headers can be set, and a body can be added, using the builder pattern:
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    // Responding to a CORS preflight request
    let r = EmptyResponse::new(204)
        .with_header("Access-Control-Allow-Methods", "GET, POST")
        .with_header("Access-Control-Allow-Origin", "https://this-origin.net")
        .with_header("Access-Control-Allow-Headers", "Content-type");
    ```
    */
    pub fn new(status: u16) -> EmptyResponse {
        EmptyResponse {
            status,
            headers: HashMap::new(),
        }
    }
    
    /**
    Adds a response header.
    
    Adding multiple headers with the same name will concatenate the added
    values in a comma-separated list:
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let mut r = EmptyResponse::new(200);
    r.add_header("Custom-header", "value0");
    r.add_header("Custom-header", "value1");
    
    assert_eq!(r.get_header("Custom-header"), Some("value0, value1"));
    ```
    */
    pub fn add_header<N, V>(&mut self, name: N, value: V)
    where
        N: Into<String>,
        V: Into<String>,
    {
        let name = name.into();
        let value = value.into();
        let name_key = (&name).to_lowercase();
        match self.headers.entry(name_key) {
            Entry::Occupied(mut oe) => {
                let old = oe.get_mut();
                (*old).value.push_str(", ");
                (*old).value.push_str(&value);
            },
            Entry::Vacant(ve) => {
                let header = HeaderValue { name, value };
                ve.insert(header);
            },
        }
    }
    
    /**
    Builder pattern method for adding a header value.
    
    Works similarly to `.add_header()`:
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let r = EmptyResponse::new(200)
        .with_header("Custom-header", "value0")
        .with_header("Custom-header", "value1");
    
    assert_eq!(r.get_header("custom-header"), Some("value0, value1"));
    ```
    */
    pub fn with_header<N, V>(self, name: N, value: V) -> EmptyResponse
    where
        N: Into<String>,
        V: Into<String>,
    {
        let mut new = self;
        new.add_header(name, value);
        new
    }
    
    /**
    Adds a `Content-type` header to this request, turning it into a
    `FullResponse`, which can have a body.
    
    Any `content-type` header explicitly set with the `.with_header()` or
    `.add_header()` methods will be overwritten and replaced with this
    value when the request is sent.
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let r = EmptyResponse::new(400)
        .with_content_type("test/plain")
        .with_body("Your request must contain a \"Content=type\" header.");
    ````
    */
    pub fn with_content_type<T>(self, content_type: T) -> FullResponse
    where
        T: Into<String>,
    {
        FullResponse {
            status: self.status,
            headers: self.headers,
            content_type: content_type.into(),
            body: Vec::new(),
        }
    }
    
    
    /// Return the HTTP status code associated with this response.
    pub fn get_status(&self) -> u16 { self.status }
    
    /// Change the HTTP status code associated with this response.
    pub fn set_status(&mut self, new_status: u16) { self.status = new_status; }
    
    /// Return the header value associated with the header `name` (if set).
    pub fn get_header<T: AsRef<str>>(&self, name: T) -> Option<&str> {
        let name = name.as_ref().to_lowercase();
        self.headers.get(&name).map(|s| s.value.as_str())
    }
    
    /**
    Write this response to stdout. This consumes the value.
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let r = EmptyResponse::new(204)
        .with_header("Status-Message", "success")
        .with_header("Cache-Control", "no-store");
    
    r.respond().unwrap();
    ```
    */
    pub fn respond(mut self) -> std::io::Result<()> {
        let status_str = format!("{}", &self.status);
        let status_header = HeaderValue { 
            name: "Status".to_owned(),
            value: status_str
        };
        _ = self.headers.insert("status".to_owned(), status_header);
        
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        for (_, header) in self.headers.iter() {
            write!(&mut out, "{}: {}\r\n", &header.name, &header.value)?;
        }
        
        write!(&mut out, "\r\n")
    }
    
}

/**
A response with a body, instantiated by calling `.with_content_type()`
on an `EmptyResponse`.

Note that there is no `FullResponse::new()` associated function, and that
the only way to get a `FullResponse` is by adding a content type to an
`EmptyResponse` with the `.with_content_type()` method.

```rust
# use dumb_cgi::EmptyResponse;
let r = EmptyResponse::new(200)                 // an `EmptyResponse` upon instantiation
    .with_header("Cache-Control", "no-store")  // still an `EmptyResponse`
    .with_content_type("text/json")            // now a `FullResponse`
    .with_body("{\"status\":\"updated\"}");

r.respond().unwrap();
```

`FullResponse` also implements `std::io::Write` for writing to the body:

```rust
# use dumb_cgi::EmptyResponse;
# use std::io::Write;
let mut r = EmptyResponse::new(200)
    .with_content_type("text/plain");

let status = r.get_status();

write!(&mut r, "This is the body of the response.\n").unwrap();
write!(&mut r, "The status is {}.", &status).unwrap();

r.respond().unwrap();
```
*/

#[derive(Debug)]
pub struct FullResponse {
    status: u16,
    headers: HashMap<String, HeaderValue>,
    body: Vec<u8>,
    content_type: String,
}

impl FullResponse {
    /**
    Adds a response header.
    
    Adding multiple headers with the same name will concatenate the added
    values in a comma-separated list:
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let mut r = EmptyResponse::new(200).with_content_type("text/plain");
    r.add_header("Custom-header", "value0");
    r.add_header("Custom-header", "value1");
    
    assert_eq!(r.get_header("Custom-header"), Some("value0, value1"));
    ```
    */
    pub fn add_header<N, V>(&mut self, name: N, value: V)
    where
        N: Into<String>,
        V: Into<String>,
    {
        let name = name.into();
        let value = value.into();
        let name_key = (&name).to_lowercase();
        match self.headers.entry(name_key) {
            Entry::Occupied(mut oe) => {
                let old = oe.get_mut();
                (*old).value.push_str(", ");
                (*old).value.push_str(&value);
            },
            Entry::Vacant(ve) => {
                let header = HeaderValue { name, value };
                ve.insert(header);
            },
        }
    }
    
    /**
    Builder pattern method for adding a header value.
    
    Works similarly to `.add_header()`:
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let r = EmptyResponse::new(200)
        .with_content_type("test/plain")
        .with_header("Custom-header", "value0")
        .with_header("Custom-header", "value1");
    
    assert_eq!(r.get_header("custom-header"), Some("value0, value1"));
    ```
    */
    pub fn with_header<N, V>(self, name : N, value: V) -> FullResponse
    where
        N: Into<String>,
        V: Into<String>,
     {
        let mut new = self;
        new.add_header(name, value);
        new
    }
    
    /**
    Builder-pattern method for adding a body.
    
    This replaces any current body value with `new_body`:
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let r = EmptyResponse::new(200)
        .with_content_type("text/plain")
        .with_body("This is the first body.")
        .with_body("This is the second body.");
    
    assert_eq!(r.get_body(), "This is the second body.".as_bytes());
    ```
    */
    pub fn with_body<T: Into<Vec<u8>>>(self, new_body: T) -> FullResponse {
        let mut new = self;
        new.body = new_body.into();
        new
    }

    /// Return the HTTP status code associated with this response.
    pub fn get_status(&self) -> u16 { self.status }
    
    /// Change the HTTP status code associated with this response.
    pub fn set_status(&mut self, new_status: u16) { self.status = new_status; }

    /// Return the header value associated with the header `name` (if set).
    pub fn get_header<T: AsRef<str>>(&self, name: T) -> Option<&str> {
        let name = name.as_ref().to_lowercase();
        self.headers.get(&name).map(|s| s.value.as_str())
    }
    
    /// Return a reference to the current body payload.
    pub fn get_body(&self) -> &[u8] { &self.body }
    
    /**
    Write this response to stdout. This consumes the value.
    
    ```rust
    # use dumb_cgi::EmptyResponse;
    let body: &str = "<!doctype html>
    <html>
    <head>
        <meta charset='utf-8'>
    </head>
    <body>
        <h1>Hello, browser!</h1>
    </body>
    </html>";
        
    let r = EmptyResponse::new(200)
        .with_content_type("text/html")    // this makes it a `FullResponse`
        .with_body(body);
        
    r.respond().unwrap();
    ```
    */
    pub fn respond(mut self) -> std::io::Result<()> {
        let status_str = format!("{}", &self.status);
        self.add_header("Status".to_owned(), status_str);
        if self.body.len() > 0 {
            self.add_header(
                "Content-type".to_owned(),
                self.content_type.clone()
            );
            self.add_header(
                "Content-length".to_owned(),
                format!("{}", self.body.len())            
            );
        }
        
        let stdout = std::io::stdout();
        let mut out = stdout.lock();
        
        for (_, header) in self.headers.iter() {
            write!(&mut out, "{}: {}\r\n", &header.name, &header.value)?;
        }
        write!(&mut out, "\r\n")?;
        
        if self.body.len() > 0 {
            out.write_all(&self.body)?;
        }
        
        Ok(())
    }
}

/// `Write` is implemented for `FullResponse` by appending to the `.body`
/// vector, in exactly the same way it's implemented for `Vec<u8>`.
impl Write for FullResponse {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.body.extend_from_slice(buf);
        Ok(buf.len())
    }
    
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}