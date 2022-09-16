const out = document.getElementById("display-latest");

async function refresh() {
    let res = await fetch("/latest");
    if (res.status >= 400)
        throw new Error(`Refresh got status code: ${res.status}`);
    out.innerText = await res.text();
    return "Request got: " + res.status;
}

function callback() {
    refresh().then(console.log)
    setTimeout(callback, 100);
}

callback()

console.log("Js file loaded");