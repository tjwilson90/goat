import init, { Client } from "./wasm/goat_wasm.js";

await init();

const client = new Client();
window.client = client;

function getCookie(name) {
    const prefix = name + "=";
    const cookies = decodeURIComponent(document.cookie).split(";");
    for (let i = 0; i < cookies.length; i++) {
        const c = cookies[i].trimStart();
        if (c.startsWith(prefix)) {
            return c.substring(prefix.length);
        }
    }
    return null;
}

export function updateGame(gameId, replay) {
    let gameElem = document.querySelector(`[data-gameId="${gameId}"]`);
    if (!gameElem) {
        gameElem = unstartedGameElement(gameId);
        document.getElementById("games").appendChild(gameElem);
    }
    const game = client.game(gameId);
    switch (game.phase.type) {
        case "unstarted":
            updateUnstartedGame(gameId, game, gameElem);
            break;
        case "war":
            updateWarGame(gameId, game, gameElem);
            break;
        case "rummy":
            updateRummyGame(gameId, game, gameElem);
            break;
        case "goat":
            updateCompleteGame(game, gameElem, replay);
            break;
    }
}

function updateUnstartedGame(gameId, game, gameElem) {
    const notPlayerIds = new Set(client.userIds());

    const playersElem = gameElem.querySelector(".players");
    const newPlayerElems = document.createDocumentFragment();
    for (let userId of game.players) {
        notPlayerIds.delete(userId);
        const playerElem = playersElem.querySelector(`[data-userId="${userId}"]`) ?? unstartedGamePlayerElement(gameId, userId);
        newPlayerElems.appendChild(playerElem);
    }
    playersElem.innerHTML = null;
    playersElem.appendChild(newPlayerElems);

    const addPlayersElem = gameElem.querySelector(".add-players");
    addPlayersElem.disabled = game.players.length >= 16;
    const newAddPlayerElems = [];
    for (let userId of notPlayerIds) {
        let addPlayerElem = addPlayersElem.querySelector(`[data-userId="${userId}"]`) ?? unstartedGameAddPlayerElement(userId);
        newAddPlayerElems.push(addPlayerElem);
    }
    const addPlayersDefaultOption = addPlayersElem.firstChild;
    addPlayersElem.innerHTML = null;
    addPlayersElem.appendChild(addPlayersDefaultOption);
    newAddPlayerElems
        .sort((a, b) => a.textContent < b.textContent ? -1 : 1)
        .forEach(child => addPlayersElem.appendChild(child));
    addPlayersElem.value = "";

    const startGameElem = gameElem.querySelector(".start-game");
    startGameElem.disabled = game.players.length < 3;
}

function updateWarGame(gameId, game, gameElem) {
    if (gameElem.dataset.phase !== "war") {
        gameElem.innerHTML = null;
        gameElem.setAttribute("data-phase", "war");
        gameElem.appendChild(warGameElement(gameId, game));
    }

    const index = game.players.indexOf(window.userId);

    const deckLenElem = gameElem.querySelector(".deck-len");
    deckLenElem.textContent = `Deck: ${game.phase.deck} card${game.phase.deck == 1 ? "" : "s"}`;

    updateTrickElements(
        gameElem.querySelectorAll(".current .trick-played"),
        gameElem.querySelectorAll(".current .trick-sloughed"),
        game.phase.currTrick
    );
    updateTrickElements(
        gameElem.querySelectorAll(".previous .trick-played"),
        gameElem.querySelectorAll(".previous .trick-sloughed"),
        game.phase.prevTrick
    );

    const handElems = gameElem.querySelectorAll(".other-hand");
    const wonElems = gameElem.querySelectorAll(".won");
    for (let i = 0; i < game.players.length; i++) {
        const hand = game.phase.hands[i];
        const handElem = handElems[i];
        if (hand.type == "hidden") {
            handElem.textContent = `Hand: ${hand.length} card${hand.length == 1 ? "" : "s"}`;
        } else if (hand.cards.length == 0) {
            handElem.textContent = "Hand: 0 cards";
        } else {
            handElem.textContent = "Hand: ";
            for (const card of hand.cards) {
                handElem.appendChild(pretty(card.card));
            }
        }
        const won = game.phase.won[i];
        const wonElem = wonElems[i];
        wonElem.textContent = `Won: ${won} card${won == 1 ? "" : "s"}`;
    }

    if (index >= 0) {
        const playTopElem = gameElem.querySelector(".play-top");
        playTopElem.disabled = game.phase.deck <= 0
            || game.phase.currTrick.next != index
            || game.phase.hands[index].cards.some(card => card.card[0] == game.phase.currTrick.rank);

        const drawElem = gameElem.querySelector(".draw");
        drawElem.disabled = game.phase.deck <= 0
            || game.phase.hands[index].cards.length >= 3;

        const finishTrickElem = gameElem.querySelector(".finish-trick");
        finishTrickElem.disabled = (!game.phase.finished && game.phase.currTrick.winner === undefined)
            || (game.phase.currTrick.endMask & (1 << index)) === 0;

        const playsElem = gameElem.querySelector(".my-plays");
        const newPlaysElem = document.createDocumentFragment();
        for (const card of game.phase.hands[index].cards) {
            const cardElem = playsElem.querySelector(`[data-card="${card}"]`) ?? warCardElement(gameId, card.card, playCard);
            cardElem.disabled = !card.playable;
            newPlaysElem.appendChild(cardElem);
        }
        playsElem.innerHTML = null;
        playsElem.appendChild(newPlaysElem);

        const sloughsElem = gameElem.querySelector(".my-sloughs");
        const newSloughsElem = document.createDocumentFragment();
        for (const card of game.phase.hands[index].cards) {
            const cardElem = sloughsElem.querySelector(`[data-card="${card}"]`) ?? warCardElement(gameId, card.card, slough);
            cardElem.disabled = !card.sloughable;
            newSloughsElem.appendChild(cardElem);
        }
        sloughsElem.innerHTML = null;
        sloughsElem.appendChild(newSloughsElem);
    }
}

function updateRummyGame(gameId, game, gameElem) {
    if (gameElem.dataset.phase !== "rummy") {
        gameElem.innerHTML = null;
        gameElem.setAttribute("data-phase", "rummy");
        gameElem.appendChild(rummyGameElement(gameId, game));
    }

    const newTrickElem = document.createDocumentFragment();
    newTrickElem.appendChild(document.createTextNode("Current Trick: "));
    for (let i = 0; i < game.phase.trick.plays.length; i++) {
        if (i != 0) {
            newTrickElem.appendChild(document.createTextNode(", "));
        }
        let play = game.phase.trick.plays[i];
        appendCardRange(newTrickElem, play[0], play[1]);
    }
    const trickElem = gameElem.querySelector(".rummy-trick");
    trickElem.innerHTML = null;
    trickElem.appendChild(newTrickElem);

    const index = game.players.indexOf(window.userId);

    const handElems = gameElem.querySelectorAll(".other-hand");
    const lastPlayElems = gameElem.querySelectorAll(".last-play");

    for (let i = 0; i < game.players.length; i++) {
        const hand = game.phase.hands[i];
        const handElem = handElems[i];
        if (index >= 0 && index != i) {
            handElem.textContent = `Hand: ${hand.length} card${hand.length == 1 ? "" : "s"}`;
        } else if (hand.length == 0) {
            handElem.textContent = "Hand: 0 cards";
        } else {
            handElem.textContent = "Hand: ";
            for (const card of hand.cards) {
                handElem.appendChild(pretty(card.card));
            }
        }
        handElem.parentElement.classList.toggle("next", i === game.phase.next);
        handElem.parentElement.classList.toggle("finished", hand.length === 0);

        updateLastPlay(lastPlayElems[i], game.phase.history[i]);
    }
    if (index >= 0) {
        const cardsElem = gameElem.querySelector(".rummy-cards");
        const newCardsElem = document.createDocumentFragment();
        for (const card of game.phase.hands[index].cards) {
            newCardsElem.appendChild(rummyCardElement(gameId, card));
        }
        cardsElem.innerHTML = null;
        cardsElem.appendChild(newCardsElem);
        updateRummyCards(gameId, game, index);
    }
}

function updateLastPlay(elem, action) {
    if (action.type === "none") {
        return;
    }
    elem.innerHTML = null;
    elem.appendChild(createElement("span", {textContent: "Last Play: "}));
    switch (action.type) {
        case "lead":
            elem.appendChild(createElement("span", {textContent: "Lead "}));
            appendCardRange(elem, action.lo, action.hi);
            break;
        case "play":
            elem.appendChild(createElement("span", {textContent: "Play "}));
            appendCardRange(elem, action.lo, action.hi);
            break;
        case "kill":
            elem.appendChild(createElement("span", {textContent: "Kill "}));
            appendCardRange(elem, action.lo, action.hi);
            break;
        case "killAndLead":
            elem.appendChild(createElement("span", {textContent: "Kill "}));
            appendCardRange(elem, action.killLo, action.killHi);
            elem.appendChild(createElement("span", {textContent: ", Lead "}));
            appendCardRange(elem, action.leadLo, action.leadHi);
            break;
        case "pickUp":
            elem.appendChild(createElement("span", {textContent: "Pick Up "}));
            appendCardRange(elem, action.lo, action.hi);
            break;
    }
}

function appendCardRange(elem, lo, hi) {
    elem.appendChild(pretty(lo));
    if (lo !== hi) {
        elem.appendChild(createElement("span", {textContent: " - "}));
        elem.appendChild(pretty(hi));
    }
}

function updateCompleteGame(game, gameElem, replay) {
    gameElem.innerHTML = null;
    gameElem.setAttribute("data-phase", "goat");
    gameElem.appendChild(document.createTextNode("Goat: "));
    const goat = game.players[game.phase.goat];
    gameElem.appendChild(nameElement(goat));
    if (!replay && game.phase.noise !== undefined) {
        const noise = new Audio(`./assets/noises/goat-${game.phase.noise}.mp3`);
        noise.play();
    }
}

function unstartedGameElement(gameId) {
    return createElement("div", {
        classList: ["game"],
        attributes: {gameId: gameId, phase: "unstarted"},
        children: [
            createElement("ul", {classList: ["players", "vertical"]}),
            unstartedGameAddPlayersElement(gameId),
            unstartedGameStartGameElement(gameId)
        ]
    });
}

function unstartedGameAddPlayersElement(gameId) {
    return createElement("select", {
        classList: ["add-players", "sorted-users"],
        listeners: {
            change: (event) => {
                if (event.target.value) {
                    joinGame(gameId, event.target.value);
                }
            }
        },
        children: [
            createElement("option", {
                value: "",
                textContent: "Add a Player"
            })
        ]
    });
}

function unstartedGameStartGameElement(gameId) {
    return createElement("select", {
        classList: ["start-game"],
        listeners: {change: (event) => startGame(gameId, event.target.value)},
        children: [
            createElement("option", {
                value: "",
                textContent: "Start Game"
            }),
            unstartedGameStartGameNumDecksElement(1),
            unstartedGameStartGameNumDecksElement(2),
            unstartedGameStartGameNumDecksElement(3)
        ]
    });
}

function unstartedGameStartGameNumDecksElement(numDecks) {
    return createElement("option", {
        value: numDecks,
        textContent: `Use ${numDecks} deck${numDecks == 1 ? "" : "s"}`
    });
}

function unstartedGamePlayerElement(gameId, userId) {
    const user = client.user(userId);
    const element = createElement("li", {
        attributes: {userId, userId},
        classList: ["name", "horizontal"],
        children: [
            createElement("span", {textContent: user.name}),
            createElement("button", {
                textContent: "X",
                listeners: {click: (event) => leaveGame(gameId, userId)}
            })
        ],
    });
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    return element;
}

function unstartedGameAddPlayerElement(userId) {
    const user = client.user(userId);
    const element = createElement("option", {
        attributes: {userId, userId},
        value: userId,
        textContent: user.name
    });
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    return element;
}

function warGameElement(gameId, game) {
    const element = document.createDocumentFragment();
    element.appendChild(createElement("p", {classList: ["deck-len"]}));
    element.appendChild(warGamePlayersElement(game));
    if (game.players.includes(window.userId)) {
        element.appendChild(warGameActionsElement(gameId));
    }
    return element;
}

function warGamePlayersElement(game) {
    return createElement("div", {
        classList: ["horizontal"],
        children: [
            createElement("div", {
                children: game.players.map(userId => warGamePlayerInfoElement(userId))
            }),
            warGameTrickElement(game, "Current"),
            warGameTrickElement(game, "Previous")
        ]
    });
}

function warGamePlayerInfoElement(userId) {
    return createElement("div", {
        classList: ["info"],
        children: [
            nameElement(userId),
            createElement("p", {classList: ["other-hand"]}),
            createElement("p", {classList: ["won"]})
        ]
    });
}

function nameElement(userId) {
    const user = client.user(userId);
    const element = createElement("p", {
        attributes: {userId, userId},
        classList: ["name"],
        textContent: user.name
    });
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    return element;
}

function warGameTrickElement(game, kind) {
    return createElement("div", {
        children: game.players.map(userId => warGamePlayerTrickElement(kind))
    });
}

function warGamePlayerTrickElement(kind) {
    return createElement("div", {
        classList: ["trick", kind.toLowerCase()],
        children: [
            createElement("p", {textContent: `${kind} trick:`}),
            createElement("p", {classList: ["trick-played"]}),
            createElement("p", {classList: ["trick-sloughed"]})
        ]
    });
}

function warGameActionsElement(gameId) {
    return createElement("div", {
        classList: ["horizontal"],
        children: [
            createElement("div", {
                classList: ["vertical"],
                children: [
                    createElement("div", {
                        classList: ["horizontal"],
                        children: [
                            createElement("button", {
                                classList: ["play-top"],
                                textContent: "Play Top",
                                listeners: {click: (event) => playTop(gameId)}
                            }),
                            createElement("button", {
                                classList: ["draw"],
                                textContent: "Draw",
                                listeners: {click: (event) => draw(gameId)}
                            })
                        ]
                    }),
                    createElement("button", {
                        classList: ["finish-trick"],
                        textContent: "Finish Trick",
                        listeners: {click: (event) => finishTrick(gameId)}
                    }),
                ]
            }),
            createElement("div", {
                classList: ["vertical"],
                children: [
                    createElement("div", {
                        classList: ["horizontal"],
                        children: [
                            createElement("span", {classList: ["plays-label"], textContent: "Plays: "}),
                            createElement("div", {classList: ["my-plays"]})
                        ]
                    }),
                    createElement("div", {
                        classList: ["horizontal"],
                        children: [
                            createElement("span", {classList: ["plays-label"], textContent: "Sloughs: "}),
                            createElement("div", {classList: ["my-sloughs"]})
                        ]
                    })
                ]
            })
        ]
    });
}

function warCardElement(gameId, card, handler) {
    return createElement("button", {
        classList: ["war-card"],
        attributes: {card: card},
        children: [pretty(card)],
        listeners: {click: (event) => handler(gameId, card)}
    });
}

function rummyGameElement(gameId, game) {
    const element = document.createDocumentFragment();
    element.appendChild(createElement("div", {
        classList: ["horizontal"],
        children: [
            createElement("p", {
                classList: ["trump"],
                children: [
                    createElement("span", {textContent: "Trump: "}),
                    pretty(game.phase.trump)
                ]
            }),
            createElement("p", {classList: ["rummy-trick"]})
        ]
    }));
    element.appendChild(rummyGamePlayersElement(game));
    if (game.players.includes(window.userId)) {
        element.appendChild(rummyGameActionsElement(gameId));
    }
    return element;
}

function rummyGamePlayersElement(game) {
    return createElement("div", {
        classList: ["vertical"],
        children: game.players.map(userId => rummyGamePlayerInfoElement(userId))
    });
}

function rummyGamePlayerInfoElement(userId) {
    return createElement("div", {
        classList: ["info"],
        children: [
            nameElement(userId),
            createElement("p", {classList: ["other-hand"]}),
            createElement("p", {classList: ["last-play"]})
        ]
    });
}

function rummyGameActionsElement(gameId) {
    return createElement("div", {
        classList: ["horizontal"],
        children: [
            createElement("button", {
                classList: ["pick-up"],
                textContent: "Pick Up",
                listeners: {click: (event) => pickUp(gameId)}
            }),
            createElement("button", {
                classList: ["play-range"],
                textContent: "Play",
                listeners: {click: (event) => playRunOne(gameId)}
            }),
            createElement("div", {classList: ["rummy-cards", "horizontal"]})
        ]
    });
}

function updateRummyCards(gameId, game, index) {
    let gameElem = document.querySelector(`[data-gameId="${gameId}"]`);
    const cardsElem = gameElem.querySelector(".rummy-cards");
    const pickUpElem = gameElem.querySelector(".pick-up");
    const playRangeElem = gameElem.querySelector(".play-range");
    if (game.phase.next != index) {
        pickUpElem.disabled = true;
        playRangeElem.disabled = true;
        for (const checkElem of gameElem.querySelectorAll(".rummy-card input")) {
            checkElem.disabled = true;
            checkElem.checked = false;
        }
        return;
    }

    pickUpElem.disabled = game.phase.trick.plays.length == 0;
    const checkedElems = gameElem.querySelectorAll(".rummy-card input:checked");
    if (checkedElems.length == 0) {
        playRangeElem.disabled = true;
        for (const cardElem of cardsElem.children) {
            const checkElem = cardElem.querySelector("input");
            checkElem.disabled = !cardElem.classList.contains("canPlay");
        }
    } else if (checkedElems.length == 1) {
        playRangeElem.disabled = false;
        const checkedCardElem = checkedElems[0].parentElement;
        for (const cardElem of cardsElem.children) {
            const checkElem = cardElem.querySelector("input");
            if (cardElem.dataset.card == checkedCardElem.dataset.card) {
                checkElem.disabled = !cardElem.isSameNode(checkedCardElem);
            } else {
                checkElem.disabled = cardElem.dataset.runmin != checkedCardElem.dataset.runmin;
            }
        }
    } else {
        playRangeElem.disabled = true;
        for (const cardElem of cardsElem.children) {
            const checkElem = cardElem.querySelector("input");
            checkElem.checked = false;
            checkElem.disabled = !cardElem.classList.contains("canPlay");
        }
    }
}

function rummyCardElement(gameId, card) {
    const element = createElement("div", {
        classList: ["rummy-card", "vertical"],
        attributes: {card: card.card, runmin: card.runMin},
        children: [
            pretty(card.card),
            createElement("input", {
                type: "checkbox",
                classList: ["card-select"],
                listeners: {click: (event) => {
                    playRunMany(gameId);
                    const game = client.game(gameId);
                    const index = game.players.indexOf(window.userId);
                    updateRummyCards(gameId, game, index);
                }}
            })
        ]
    });
    element.classList.toggle("canPlay", card.canPlay);
    return element;
}

function createElement(tagName, options) {
    const element = document.createElement(tagName);
    for (const [key, value] of Object.entries(options)) {
        switch (key) {
            case "classList":
                for (const clazz of value) {
                    element.classList.add(clazz);
                }
                break;
            case "attributes":
                for (const [k, v] of Object.entries(value)) {
                    element.setAttribute("data-" + k, v);
                }
                break;
            case "children":
                for (const child of value) {
                    element.appendChild(child);
                }
                break;
            case "listeners":
                for (const [kind, handler] of Object.entries(value)) {
                    element.addEventListener(kind, handler);
                }
                break;
            default:
                element[key] = value
        }
    }
    return element;
}

function updateTrickElements(playElements, sloughElements, trick) {
    for (const element of playElements) {
        element.innerHTML = "Plays: ";
    }
    for (const element of sloughElements) {
        element.innerHTML = "Sloughs: ";
    }
    if (!trick) {
        return;
    }
    for (let i = 0; i < playElements.length; i++) {
        const next = trick.winner === undefined ? i === trick.next : (trick.endMask & (1 << i)) !== 0;
        playElements[i].parentElement.classList.toggle("next", next);
    }
    for (const play of trick.plays) {
        const parent = play.kind == "slough" ? sloughElements[play.player] : playElements[play.player];
        const child = pretty(play.card);
        child.classList.add(play.kind);
        child.classList.toggle("lead", play.lead);
        parent.appendChild(child);
    }
}

const RANKS = {
    "2": "2",
    "3": "3",
    "4": "4",
    "5": "5",
    "6": "6",
    "7": "7",
    "8": "8",
    "9": "9",
    "T": "10",
    "J": "J",
    "Q": "Q",
    "K": "K",
    "A": "A"
}

const SUITS = {
    "C": "♣",
    "D": "♦",
    "H": "♥",
    "S": "♠",
}

function pretty(card) {
    return createElement("span", {
        attributes: {suit: card[1]},
        textContent: RANKS[card[0]] + SUITS[card[1]]
    });
}

export function forgetGame(gameId) {
    const gameNodes = document.querySelectorAll(`[data-gameId="${gameId}"]`);
    for (const gameNode of gameNodes) {
        gameNode.remove();
    }
}

export function updateUser(userId, user) {
    let userNodes = document.querySelectorAll(`[data-userId="${userId}"]`);
    if (userNodes.length == 0) {
        const userNode = createElement("li", {
            classList: ["name"],
            attributes: {userId, userId}
        });
        document.getElementById("subscribers").appendChild(userNode);
        userNodes = [userNode];
    }
    for (const userNode of userNodes) {
        userNode.classList.toggle("online", user.online);
        userNode.classList.toggle("self", userId === window.userId);
        const textNode = userNode.querySelector("span") ?? userNode;
        textNode.textContent = user.name;
    }
    for (const userContainerNode of document.querySelectorAll(".sorted-users")) {
        [...userContainerNode.children]
            .filter(child => child.hasAttribute("data-userId"))
            .sort((a, b) => a.firstChild.textContent < b.firstChild.textContent ? -1 : 1)
            .forEach(child => userContainerNode.appendChild(child));
    }
}

export function forgetUser(userId) {
    const userNodes = document.querySelectorAll(`[data-userId="${userId}"]`);
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

export function finishTrick(gameId) {
    applyAction(gameId, `{"type":"finishTrick"}`);
}

export function playRunOne(gameId) {
    const checkedElems = document.querySelectorAll(`[data-gameId="${gameId}"] .rummy-card input:checked`)
    const card = checkedElems[0].parentElement.dataset.card;
    disableButtons(gameId);
    applyAction(gameId, `{"type":"playRun","lo":"${card}","hi":"${card}"}`);
}

export function playRunMany(gameId) {
    const checkedElems = document.querySelectorAll(`[data-gameId="${gameId}"] .rummy-card input:checked`)
    if (checkedElems.length >= 2) {
        const lo = checkedElems[0].parentElement.dataset.card;
        const hi = checkedElems[1].parentElement.dataset.card;
        applyAction(gameId, `{"type":"playRun","lo":"${lo}","hi":"${hi}"}`);
    }
}

export function pickUp(gameId) {
    disableButtons(gameId);
    applyAction(gameId, `{"type":"pickUp"}`);
}

function disableButtons(gameId) {
    let gameElem = document.querySelector(`[data-gameId="${gameId}"]`);
    gameElem.querySelector(".pick-up").disabled = true;
    gameElem.querySelector(".play-range").disabled = true;
    for (const checkElem of gameElem.querySelectorAll(".rummy-card input")) {
        checkElem.disabled = true;
        checkElem.checked = false;
    }
}

function applyAction(gameId, action) {
    fetch(`./apply_action?game_id=${gameId}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: action,
    });
}

function signalUpdate() {
    if (document.visibilityState !== "visible") {
        document.title = "* Goat";
    }
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

document.getElementById("rules").addEventListener("click", (event) => {
    alert("1. Loser must make a goat noise.\n2. No free shows.\n3. Other rules must be figured out as you play.");
});

document.addEventListener("visibilitychange", (event) => {
    if (document.visibilityState === "visible") {
        document.title = "Goat";
    }
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
        case "game":
            updateGame(response.gameId, false);
            signalUpdate();
            break;
        case "replay":
            updateGame(response.gameId, true);
            break;
        case "forgetGame":
            forgetGame(response.gameId);
            break;
        case "user":
            updateUser(response.userId, response.user);
            signalUpdate();
            break;
        case "forgetUser":
            forgetUser(response.userId);
            break;
    }
}
