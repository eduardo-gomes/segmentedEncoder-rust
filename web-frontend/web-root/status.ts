const status_div = document.getElementById("status");
const out = document.createElement("pre");
status_div.appendChild(out);

async function refresh() {
    let res = await fetch("/latest");
    if (res.status >= 400)
        throw new Error(`Refresh got status codee: ${res.status}`);
    out.innerText = await res.text();
    return "Request got: " + res.status;
}

function callback(timeout: number) {
    refresh().then(console.log)
    setTimeout(callback, timeout, timeout);
}

export {callback as status_updater};