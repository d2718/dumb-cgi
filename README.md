# dumb-cgi
An adequate CGI library in Rust.

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


## To Do

  * More stress testing.
  * Perhaps implement a `Response` type to make writing responses easier.