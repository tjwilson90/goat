import init, { PartialGame, JsEvent } from './assets/wasm/goat_wasm.js';

const userNames = new Map();
const games = new Map();

function getCookie(cname) {
    let name = cname + "=";
    let decodedCookie = decodeURIComponent(document.cookie);
    let ca = decodedCookie.split(';');
    for (let i = 0; i < ca.length; i++) {
        let c = ca[i];
        while (c.charAt(0) == ' ') {
            c = c.substring(1);
        }
        if (c.indexOf(name) == 0) {
            return c.substring(name.length, c.length);
        }
    }
    return "";
}

function uuidv4() {
    return ([1e7]+-1e3+-4e3+-8e3+-1e11).replace(/[018]/g, c =>
        (c ^ crypto.getRandomValues(new Uint8Array(1))[0] & 15 >> c / 4).toString(16)
    );
}

function userId() {
    let userId = getCookie("USER_ID");
    if (!userId) {
        userId = uuidv4();
        document.cookie = "USER_ID=" + userId;
    }
    return userId;
}

function userName() {
    let userName = getCookie("USER_NAME");
    if (!userName) {
        userName = "Anonymous";
        document.cookie = "USER_NAME=" + userName;
    }
    return userName;
}

function subscribe() {
    const eventSource = new EventSource("./subscribe");
    eventSource.onmessage = function(event) {
        const data = JSON.parse(event.data);
        switch (data.type) {
            case "replay":
                game = new PartialGame();
                games.set(data.game_id, game);
            case "changeName":
                userNames.set(data.userId, data.name);
                break;
            case "disconnect":
                userNames.delete(data.userId);
                break;
        }
    }
    return eventSource;
}

function start() {
    userId();
    const nameField = document.getElementById("name");
    nameField.value = userName();
    nameField.addEventListener("change", (event) => {
        document.cookie = "USER_NAME=" + nameField.value;
        fetch("./change_name", { method: "POST" });
    });
    subscribe();
}

await init();
start();
