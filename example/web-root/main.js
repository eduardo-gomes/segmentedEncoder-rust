const out = document.getElementById("display-latest");

async function refresh(){
    let res = await fetch("/latest");
    let text = await res.text();
    console.log(res, text);
    out.innerText = text;
    return "Request got: " + res.status;
}

function callback(){
    refresh().then(console.log)
    setTimeout(callback, 500);
}
callback()

console.log("Js file loaded");