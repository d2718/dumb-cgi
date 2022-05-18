/*
requester.js

Javascript for implementing request exploration test harness
*/
const VERSION = "3.4";

const OUTPUT = document.getElementById("output");
const STATUS = document.getElementById("status");

function clear_elt(elt) {
  while (elt.firstChild) {
    clear_elt(elt.lastChild);
    elt.removeChild(elt.lastChild);
  }
}

function set_output(txt) {
  clear_elt(OUTPUT);
  var processed = txt.replaceAll("\n", "⏎\n");
  processed = processed.replaceAll("\r", "¬");
  const text_node = document.createTextNode(processed);
  OUTPUT.appendChild(text_node);
}

function set_status(txt) {
  clear_elt(STATUS);
  const text_node = document.createTextNode(txt);
  STATUS.appendChild(text_node);
}

function submit_form(elt) {
  const form = elt.target.form;
  const submit_method = form.method;
  var data = new FormData(form);
  var uri = document.querySelector('input[name = "endpoint"]:checked').value;
  const request_object = { method: submit_method };
  
  console.log(form, form.method)
  
  if(form.method == "get") {
    data = new URLSearchParams(data);
    uri = `${uri}?${data.toString()}`;
  } else {
    request_object.body = data;
  }
  
  console.log("fetching", uri, request_object);
  
  set_status(`fetching: ${uri}`);
  
  fetch(uri, request_object)
  .then(r => {
    r.text()
    .then(t => {
      set_status(`Response status: ${String(r.status)}`);
      set_output(t);
    })
  })
  .catch(e => {
    set_status(String(e));
  });
}

function init() {
  const submitters = document.querySelectorAll("form button");
  for(const butt of submitters) {
    butt.addEventListener("click", submit_form);
  }
  
  document.getElementById("jsv").innerHTML = VERSION;
}

init();