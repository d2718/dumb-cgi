/*
requester.js

Javascript for implementing request exploration test harness
*/
const VERSION = "2.1";

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
  const data = new FormData(form);
  const uri = document.querySelector('input[name = "endpoint"]:checked').value;
  
  set_status(`fetching: ${uri}`);
  
  fetch(uri, { method: "POST", body: data })
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