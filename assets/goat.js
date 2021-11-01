import init, { Client } from './wasm/goat_wasm.js';

await init();

const client = new Client();
window.client = client;

function getCookie(name) {
    const prefix = name + "=";
    const cookies = decodeURIComponent(document.cookie).split(';');
    for (let i = 0; i < cookies.length; i++) {
        const c = cookies[i].trimStart();
        if (c.startsWith(prefix)) {
            return c.substring(prefix.length);
        }
    }
    return null;
}

export function updateGame(gameId) {
    let gameElem = document.querySelector(`[data-game-id="${gameId}"]`);
    if (!gameElem) {
        gameElem = unstartedGameElement(gameId);
        document.getElementById("games").appendChild(gameElem);
    }
    let game = client.game(gameId);
    switch (game.phase.type) {
        case "unstarted":
            let notPlayerIds = new Set(client.userIds());

            let playersElem = gameElem.querySelector(".players");
            let newPlayerElems = document.createDocumentFragment();
            for (let userId of game.players) {
                notPlayerIds.delete(userId);
                let playerElem = playersElem.querySelector(`[data-user-id="${userId}"]`) ?? unstartedGamePlayerElement(gameId, userId);
                newPlayerElems.appendChild(playerElem);
            }
            playersElem.innerHTML = null;
            playersElem.appendChild(newPlayerElems);

            let addPlayersElem = gameElem.querySelector(".add-players");
            addPlayersElem.disabled = game.players.length >= 16;
            let newAddPlayerElems = [];
            for (let userId of notPlayerIds) {
                let addPlayerElem = addPlayersElem.querySelector(`[data-user-id="${userId}"]`) ?? unstartedGameAddPlayerElement(userId);
                newAddPlayerElems.push(addPlayerElem);
            }
            const addPlayersDefaultOption = addPlayersElem.firstChild;
            addPlayersElem.innerHTML = null;
            addPlayersElem.appendChild(addPlayersDefaultOption);
            newAddPlayerElems
                .sort((a, b) => a.textContent < b.textContent ? -1 : 1)
                .forEach(child => addPlayersElem.appendChild(child));
            addPlayersElem.value = "";

            let startGameElem = gameElem.querySelector(".start-game");
            startGameElem.disabled = game.players.length < 3;
            break;
        case "war":
            if (gameElem.dataset.phase !== "war") {
                gameElem.innerHTML = null;
                gameElem.setAttribute("data-phase", "war");
                gameElem.appendChild(warGameElement(gameId, game));
            }
            break;
        case "rummy":
            break;
        case "complete":
            break;
    }
}

function unstartedGameElement(gameId) {
    const element = document.createElement("div");
    element.classList.add("game");
    element.setAttribute("data-game-id", gameId);
    element.setAttribute("data-phase", "unstarted");
    element.appendChild(unstartedGamePlayersElement());
    element.appendChild(unstartedGameAddPlayersElement(gameId));
    element.appendChild(unstartedGameStartGameElement(gameId));
    return element;
}

function unstartedGamePlayersElement() {
    const element = document.createElement("ul");
    element.classList.add("players");
    return element;
}

function unstartedGameAddPlayersElement(gameId) {
    const element = document.createElement("select");
    element.classList.add("add-players");
    element.classList.add("sorted-users");
    element.addEventListener("change", (event) => {
        if (event.target.value) {
            joinGame(gameId, event.target.value);
        }
    });
    const defaultOption = document.createElement("option");
    defaultOption.value = "";
    defaultOption.textContent = "Add a Player";
    element.appendChild(defaultOption);
    return element;
}

function unstartedGameStartGameElement(gameId) {
    const element = document.createElement("select");
    element.classList.add("start-game");
    element.disabled = true;
    element.addEventListener("change", (event) => startGame(gameId, event.target.value));
    element.appendChild(unstartedGameStartGameDefaultOptionElement());
    for (let numDecks = 1; numDecks <= 3; numDecks++) {
        element.appendChild(unstartedGameStartGameNumDecksElement(numDecks));
    }
    return element;
}

function unstartedGameStartGameDefaultOptionElement() {
    const element = document.createElement("option");
    element.value = "";
    element.textContent = "Start Game";
    return element;
}

function unstartedGameStartGameNumDecksElement(numDecks) {
    const element = document.createElement("option");
    element.value = numDecks;
    element.textContent = `Use ${numDecks} deck${numDecks == 1 ? "" : "s"}`;
    return element;
}

function unstartedGamePlayerElement(gameId, userId) {
    const element = document.createElement("li");
    element.setAttribute("data-user-id", userId);
    element.classList.add("name");
    element.appendChild(document.createElement("p"));

    const user = client.user(userId);
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    element.querySelector("p").textContent = user.name;

    const leaveButton = document.createElement("button");
    leaveButton.innerHTML = "X";
    leaveButton.addEventListener("click", (event) => leaveGame(gameId, userId));
    element.appendChild(leaveButton);
    return element;
}

function unstartedGameAddPlayerElement(userId) {
    const element = document.createElement("option");
    element.setAttribute("data-user-id", userId);
    element.value = userId;

    const user = client.user(userId);
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    element.textContent = user.name;
    return element;
}

function warGameElement(gameId, game) {
    const element = document.createDocumentFragment();
    element.appendChild(warGameInfoElement(gameId));
    for (let userId of game.players) {
        element.appendChild(warGamePlayerElement(gameId, userId));
    }
    element.appendChild(warPlayCardElement(gameId));
    element.appendChild(warPlayTopElement(gameId));
    element.appendChild(warSloughCardElement(gameId));
    element.appendChild(warDrawElement(gameId));
    return element;
}

function warGameInfoElement(gameId) {
    const element = document.createElement("div");
    element.classList.add("war-info");
    return element;
}

function warGamePlayerElement(gameId, userId) {
    const element = document.createElement("div");
    element.setAttribute("data-user-id", userId);
    return element;
}

function warPlayCardElement(gameId) {
    const element = document.createElement("select");
    element.classList.add("play-card");
    element.addEventListener("change", (event) => {
        if (event.target.value) {
            playCard(gameId, event.target.value);
        }
    });
    const defaultOption = document.createElement("option");
    defaultOption.value = "";
    defaultOption.textContent = "Play a Card";
    element.appendChild(defaultOption);
    return element;
}

function warPlayTopElement(gameId) {
    const element = document.createElement("button");
    element.type = "button";
    element.classList.add("play-top");
    element.textContent = "Play Top"
    element.addEventListener("click", (event) => playTop(gameId));
    return element;
}

function warSloughCardElement(gameId) {
    const element = document.createElement("select");
    element.classList.add("slough");
    element.addEventListener("change", (event) => {
        if (event.target.value) {
            slough(gameId, event.target.value);
        }
    });
    const defaultOption = document.createElement("option");
    defaultOption.value = "";
    defaultOption.textContent = "Slough a Card";
    element.appendChild(defaultOption);
    return element;
}

function warDrawElement(gameId) {
    const element = document.createElement("button");
    element.type = "button";
    element.classList.add("draw");
    element.textContent = "Draw"
    element.addEventListener("click", (event) => draw(gameId));
    return element;
}

export function forgetGame(gameId) {
    const gameNodes = document.querySelectorAll(`[data-game-id="${gameId}"]`);
    for (const gameNode of gameNodes) {
        gameNode.remove();
    }
}

export function updateUser(userId, user) {
    let userNodes = document.querySelectorAll(`[data-user-id="${userId}"]`);
    if (userNodes.length == 0) {
        const userNode = document.createElement("li");
        userNode.setAttribute("data-user-id", userId);
        userNode.classList.add("name");
        userNode.appendChild(document.createElement("p"));
        document.getElementById("subscribers").appendChild(userNode);
        userNodes = [userNode];
    }
    for (const userNode of userNodes) {
        userNode.classList.toggle("online", user.online);
        userNode.classList.toggle("self", userId === window.userId);
        if (userNode.nodeName === "OPTION") {
            userNode.textContent = user.name;
        } else {
            userNode.querySelector("p").textContent = user.name;
        }
    }
    for (const userContainerNode of document.querySelectorAll(".sorted-users")) {
        [...userContainerNode.children]
            .filter(child => child.hasAttribute("data-user-id"))
            .sort((a, b) => a.firstChild.textContent < b.firstChild.textContent ? -1 : 1)
            .forEach(child => userContainerNode.appendChild(child));
    }
}

export function forgetUser(userId) {
    const userNodes = document.querySelectorAll(`[data-user-id="${userId}"]`);
    for (const userNode of userNodes) {
        userNode.remove();
    }
}

export function joinGame(gameId, userId) {
    applyAction(gameId, `{"type":"join","userId":"${userId}"}`);
}

export function leaveGame(gameId, userId) {
    const player = client.game(gameId).players.indexOf(userId);
    applyAction(gameId, `{"type":"leave","player":${player}}`);
}

export function startGame(gameId, numDecks) {
    applyAction(gameId, `{"type":"start","numDecks":${numDecks}}`);
}

export function playCard(gameId, card) {
    applyAction(gameId, `{"type":"playCard","card":"${card}"}`);
}

export function playTop(gameId) {
    applyAction(gameId, `{"type":"playTop"}`);
}

export function slough(gameId, card) {
    applyAction(gameId, `{"type":"slough","card":"${card}"}`);
}

export function draw(gameId) {
    applyAction(gameId, `{"type":"draw"}`);
}

export function playRun(gameId, lo, hi) {
    applyAction(gameId, `{"type":"slough","lo":"${lo}","hi":"${hi}"}`);
}

export function pickUp(gameId) {
    applyAction(gameId, `{"type":"pickUp"}`);
}

function applyAction(gameId, action) {
    fetch(`./apply_action?game_id=${gameId}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: action,
    });
}

document.getElementById("name").addEventListener("change", (event) => {
    if (event.target.value) {
        document.cookie = "USER_NAME=" + event.target.value;
        fetch("./change_name", { method: "POST" });
        event.target.value = "";
    }
});

document.getElementById("new-game").addEventListener("click", (event) => {
    fetch("./new_game", { method: "POST" });
});

if (getCookie("USER_SECRET") === null) {
    document.cookie = `USER_SECRET=${btoa(String.fromCharCode.apply(null, crypto.getRandomValues(new Uint8Array(16))))}`;
}
if (getCookie("USER_NAME") === null) {
    document.cookie = "USER_NAME=Anonymous";
}

new EventSource("./subscribe").onmessage = function(event) {
    if (!window.userId) {
        window.userId = getCookie("USER_ID");
    }
    const response = JSON.parse(event.data);
    client.apply(response);
    switch (response.type) {
        case "replay":
        case "game":
            updateGame(response.gameId);
            break;
        case "forgetGame":
            forgetGame(response.gameId);
            break;
        case "user":
            updateUser(response.userId, response.user);
            break;
        case "forgetUser":
            forgetUser(response.userId);
            break;
    }
}
