# dumb-cgi
An adequate, essentially dependencyless CGI library in Rust.

The purpose of this library is to allow server-side CGI programs to easily
parse requests (in particular "multipart/formdata" requests) without pulling
in a bunch of full-featured crates. `dumb_cgi` does not attempt (or at least
won't start off attempting) to be resource-efficient; its chief goals are
simplicity and ease of use. Some trade-offs it makes are:

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

  * Its intended use case is server-side CGI programs only. It supports
    _reading_ requests, but not making them, and only supports the parts
    of the HTTP-verse directly related to reading and parsing CGI requests.


## To Do

  * More stress testing.
  * ~~Handle GET requests more intentionally; specifically, parse the
    query string.~~ done in v 0.4.0
  * Perhaps implement a `Response` type to make writing responses easier.


## Notes

  * v 0.3.0: Removed dependence on
    [`lua-patterns`](https://crates.io/crates/lua-patterns),
    because even though I like the idea of it, and it has worked well for
    me in other projects, it kept panicing. `dumb_cgi` now depends only
    on the [`log`](https://crates.io/crates/log) logging facade (and
    [`simplelog`](https://crates.io/crates/simplelog) if you actually
    want to do some logging and enable the `log` feature).
    
  * v 0.4.0: Added explicit query string parsing.