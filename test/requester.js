/*
requester.js

Javascript for implementing request exploration test harness
*/
const URI = "https://d2718.net/cgi-bin/cgit.cgi"

const OUTPUT = document.getElementById("output");

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

function submit_form(elt) {
  const form = elt.target.form;
  const data = new FormData(form);
  
  fetch(URI, { method: "POST", body: data })
  .then(r => {
    r.text()
    .then(t => {
      console.log(r.status);
      set_output(t);
    })
  })
  .catch(console.log);
}

function init() {
  let submitters = document.querySelectorAll("form button");
  for(const butt of submitters) {
    butt.addEventListener("click", submit_form);
  }
}

init();