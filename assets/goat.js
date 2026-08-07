import init, { Client } from "./wasm/goat_wasm.js";

await init();

const client = new Client();
window.client = client;

let selectedGameId = null;
let pendingGameFocusId = null;

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

function updateEmptyState() {
    document.getElementById("empty-state").hidden = document.getElementById("game-list").children.length > 0;
}

function gameElement(gameId) {
    return [...document.getElementById("games").children]
        .find(element => element.getAttribute("data-gameId") === gameId);
}

function gameSummaryElementFor(gameId) {
    return [...document.getElementById("game-list").children]
        .find(element => element.getAttribute("data-gameSummary") === gameId);
}

function phaseLabel(game) {
    switch (game.phase.type) {
        case "unstarted": return "Waiting for players";
        case "war": return "War round";
        case "rummy": return "Rummy round";
        case "goat": return "Complete";
    }
}

function gameLabel(gameId) {
    return `Game ${gameId.slice(0, 6)}`;
}

function createGameSummaryElement(gameId) {
    return createElement("button", {
        type: "button",
        classList: ["game-summary"],
        attributes: {gameSummary: gameId},
        listeners: {click: () => showGame(gameId, true)},
        children: [
            createElement("span", {classList: ["game-summary-title"]}),
            createElement("span", {classList: ["game-summary-status"]}),
            createElement("span", {classList: ["game-summary-players"]}),
            createElement("span", {classList: ["game-summary-arrow"], textContent: "→"})
        ]
    });
}

function updateGameSummary(gameId, game) {
    let summary = gameSummaryElementFor(gameId);
    if (!summary) {
        summary = createGameSummaryElement(gameId);
        document.getElementById("game-list").appendChild(summary);
        updateEmptyState();
    }
    summary.querySelector(".game-summary-title").textContent = gameLabel(gameId);
    summary.querySelector(".game-summary-status").textContent = phaseLabel(game);
    const names = game.players.map(userId => client.user(userId).name);
    summary.querySelector(".game-summary-players").textContent = names.length > 0
        ? names.join(", ")
        : "No players yet";
}

function showGame(gameId, focusHeading = false) {
    if (focusHeading) {
        pendingGameFocusId = gameId;
    }
    const activeGame = gameElement(gameId);
    selectedGameId = gameId;
    if (!activeGame) {
        return;
    }
    document.getElementById("game-list-view").hidden = true;
    document.getElementById("game-detail").hidden = false;
    const heading = document.getElementById("game-detail-heading");
    heading.textContent = gameLabel(gameId);
    for (const game of document.getElementById("games").children) {
        game.hidden = game !== activeGame;
    }
    if (pendingGameFocusId === gameId) {
        pendingGameFocusId = null;
        heading.focus();
    }
}

function showGameList(focusHeading = false) {
    selectedGameId = null;
    pendingGameFocusId = null;
    document.getElementById("game-detail").hidden = true;
    document.getElementById("game-list-view").hidden = false;
    if (focusHeading) {
        document.getElementById("games-heading").focus();
    }
}

export function updateGame(gameId, replay) {
    let gameElem = gameElement(gameId);
    if (!gameElem) {
        gameElem = unstartedGameElement(gameId);
        gameElem.hidden = selectedGameId !== gameId;
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
            updateCompleteGame(gameId, game, gameElem, replay);
            break;
    }
    updateGameSummary(gameId, game);
    if (selectedGameId === gameId) {
        showGame(gameId);
    }
}

function updateUnstartedGame(gameId, game, gameElem) {
    const notPlayerIds = new Set(client.userIds());
    const setupPending = gameElem.hasAttribute("data-setup-pending");

    const playersElem = gameElem.querySelector(".players");
    const newPlayerElems = document.createDocumentFragment();
    for (let userId of game.players) {
        notPlayerIds.delete(userId);
        const playerElem = playersElem.querySelector(`[data-userId="${userId}"]`) ?? unstartedGamePlayerElement(gameId, userId);
        newPlayerElems.appendChild(playerElem);
    }
    playersElem.innerHTML = null;
    playersElem.appendChild(newPlayerElems);
    for (const button of playersElem.querySelectorAll("button")) {
        button.disabled = setupPending;
    }

    const addPlayersElem = gameElem.querySelector(".add-players");
    const playerLimitReached = game.players.length >= 16;
    const newAddPlayerElems = [];
    for (let userId of notPlayerIds) {
        let addPlayerElem = addPlayersElem.querySelector(`[data-userId="${userId}"]`) ?? unstartedGameAddPlayerElement(gameId, userId);
        addPlayerElem.disabled = playerLimitReached || setupPending;
        newAddPlayerElems.push(addPlayerElem);
    }
    newAddPlayerElems
        .sort((a, b) => a.textContent.localeCompare(b.textContent) || a.dataset.userid.localeCompare(b.dataset.userid));
    addPlayersElem.replaceChildren(...newAddPlayerElems);

    gameElem.querySelector(".player-count").textContent = `Players: ${game.players.length} / 16`;
    const addPlayerStatus = gameElem.querySelector(".add-player-status");
    addPlayerStatus.textContent = playerLimitReached
        ? "Player limit reached."
        : newAddPlayerElems.length === 0 ? "No other players available." : "";
    addPlayerStatus.hidden = !addPlayerStatus.textContent;

    const playersNeeded = Math.max(0, 3 - game.players.length);
    for (const button of gameElem.querySelectorAll(".start-game-actions button")) {
        button.disabled = playersNeeded > 0 || setupPending;
    }
    const startStatus = gameElem.querySelector(".start-game-status");
    startStatus.textContent = playersNeeded > 0
        ? `Add ${playersNeeded} more player${playersNeeded === 1 ? "" : "s"} to start.`
        : "";
    startStatus.hidden = !startStatus.textContent;
}

function updateWarGame(gameId, game, gameElem) {
    if (gameElem.dataset.phase !== "war") {
        gameElem.removeAttribute("data-setup-pending");
        gameElem.innerHTML = null;
        gameElem.setAttribute("data-phase", "war");
        gameElem.appendChild(warGameElement(gameId, game));
    }

    const index = game.players.indexOf(window.userId);

    const deckLenElem = gameElem.querySelector(".deck-len");
    deckLenElem.textContent = `Deck: ${game.phase.deck} card${game.phase.deck == 1 ? "" : "s"}`;

    updateTableTrick(
        gameElem.querySelector(".table-trick"),
        gameElem.querySelectorAll(".seat"),
        game,
        game.phase.currTrick
    );

    const handElems = gameElem.querySelectorAll(".other-hand");
    const wonElems = gameElem.querySelectorAll(".won");
    for (let i = 0; i < game.players.length; i++) {
        const hand = game.phase.hands[i];
        const handLength = hand.type == "hidden" ? hand.length : hand.cards.length;
        const handElem = handElems[i];
        handElem.textContent = `Hand: ${handLength} card${handLength == 1 ? "" : "s"}`;
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

        const handElem = gameElem.querySelector(".my-war-hand");
        const newHandElem = document.createDocumentFragment();
        for (const card of game.phase.hands[index].cards) {
            const cardElem = handElem.querySelector(`[data-card="${card.card}"]`)
                ?? warHandCardElement(gameId, card.card);
            cardElem.querySelector(".play-card").disabled = !card.playable;
            cardElem.querySelector(".slough-card").disabled = !card.sloughable;
            newHandElem.appendChild(cardElem);
        }
        handElem.innerHTML = null;
        handElem.appendChild(newHandElem);
    }
}

function updateRummyGame(gameId, game, gameElem) {
    if (gameElem.dataset.phase !== "rummy") {
        gameElem.innerHTML = null;
        gameElem.setAttribute("data-phase", "rummy");
        gameElem.appendChild(rummyGameElement(gameId, game));
    }

    const trickElem = gameElem.querySelector(".rummy-trick");
    trickElem.innerHTML = null;
    for (const [lo, hi] of game.phase.trick.plays) {
        for (const card of cardsInRange(lo, hi)) {
            trickElem.appendChild(createElement("div", {
                classList: ["trick-card"],
                children: [pretty(card)]
            }));
        }
    }

    const index = game.players.indexOf(window.userId);

    const handElems = gameElem.querySelectorAll(".other-hand");
    const lastPlayElems = gameElem.querySelectorAll(".last-play");

    for (let i = 0; i < game.players.length; i++) {
        const hand = game.phase.hands[i];
        const handElem = handElems[i];
        handElem.textContent = `Hand: ${hand.length} card${hand.length == 1 ? "" : "s"}`;
        const seatElem = handElem.closest(".seat");
        seatElem.classList.toggle("next", i === game.phase.next);
        seatElem.classList.toggle("finished", hand.length === 0);

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
    elem.appendChild(createElement("span", {
        classList: ["last-play-label"],
        textContent: "Last Play:"
    }));
    switch (action.type) {
        case "lead":
            elem.appendChild(lastPlayPart("Lead", action.lo, action.hi));
            break;
        case "play":
            elem.appendChild(lastPlayPart("Play", action.lo, action.hi));
            break;
        case "kill":
            elem.appendChild(lastPlayPart("Kill", action.lo, action.hi));
            break;
        case "killAndLead":
            elem.appendChild(lastPlayPart("Kill", action.killLo, action.killHi, ","));
            elem.appendChild(lastPlayPart("Lead", action.leadLo, action.leadHi));
            break;
        case "pickUp":
            elem.appendChild(lastPlayPart("Pick Up", action.lo, action.hi));
            break;
    }
}

function lastPlayPart(label, lo, hi, suffix = "") {
    const children = [
        document.createTextNode(`${label} `),
        pretty(lo)
    ];
    if (lo !== hi) {
        children.push(document.createTextNode(" – "), pretty(hi));
    }
    if (suffix) {
        children.push(document.createTextNode(suffix));
    }
    return createElement("span", {
        classList: ["last-play-part"],
        children
    });
}

function updateCompleteGame(gameId, game, gameElem, replay) {
    gameElem.innerHTML = null;
    gameElem.setAttribute("data-phase", "goat");
    gameElem.appendChild(document.createTextNode("Goat: "));
    const goat = game.players[game.phase.goat];
    gameElem.appendChild(nameElement(goat));
    if (!replay && selectedGameId === gameId && game.phase.noise !== undefined) {
        const noise = new Audio(`./assets/noises/goat-${game.phase.noise}.mp3`);
        noise.play();
    }
}

function unstartedGameElement(gameId) {
    return createElement("div", {
        classList: ["game"],
        attributes: {gameId: gameId, phase: "unstarted"},
        children: [
            createElement("p", {
                classList: ["player-count"],
            }),
            createElement("ul", {classList: ["players", "vertical"]}),
            unstartedGameAddPlayersElement(gameId),
            unstartedGameStartGameElement(gameId)
        ]
    });
}

function unstartedGameAddPlayersElement(gameId) {
    return createElement("fieldset", {
        classList: ["setup-group"],
        children: [
            createElement("legend", {textContent: "Add a player"}),
            createElement("div", {classList: ["add-players", "sorted-users"]}),
            createElement("p", {
                classList: ["setup-status", "add-player-status"],
                hidden: true
            })
        ]
    });
}

function unstartedGameStartGameElement(gameId) {
    return createElement("fieldset", {
        classList: ["setup-group"],
        children: [
            createElement("legend", {textContent: "Start game"}),
            createElement("div", {
                classList: ["start-game-actions"],
                children: [1, 2, 3].map(numDecks => createElement("button", {
                    type: "button",
                    textContent: `Start with ${numDecks} deck${numDecks === 1 ? "" : "s"}`,
                    listeners: {click: event => runSetupAction(
                        event.currentTarget,
                        () => startGame(gameId, numDecks),
                        "The game could not be started.",
                        true
                    )}
                }))
            }),
            createElement("p", {
                classList: ["setup-status", "start-game-status"],
            })
        ]
    });
}

function unstartedGamePlayerElement(gameId, userId) {
    const user = client.user(userId);
    const element = createElement("li", {
        attributes: {userId},
        classList: ["name", "horizontal"],
        children: [
            ...nameChildren(user),
            createElement("button", {
                type: "button",
                textContent: "Remove",
                listeners: {click: event => runSetupAction(
                    event.currentTarget,
                    () => leaveGame(gameId, userId),
                    `${user.name} could not be removed from the game.`
                )}
            })
        ],
    });
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    return element;
}

function unstartedGameAddPlayerElement(gameId, userId) {
    const user = client.user(userId);
    const element = createElement("button", {
        type: "button",
        classList: ["add-player", "name"],
        attributes: {userId},
        children: nameChildren(user),
        listeners: {click: event => runSetupAction(
            event.currentTarget,
            () => joinGame(gameId, userId),
            `${user.name} could not be added to the game.`
        )}
    });
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    return element;
}

function warGameElement(gameId, game) {
    const isPlayer = game.players.includes(window.userId);
    const element = document.createDocumentFragment();
    element.appendChild(warGamePlayersElement(gameId, game, isPlayer));
    if (isPlayer) {
        element.appendChild(warGameActionsElement(gameId));
    }
    return element;
}

function warGamePlayersElement(gameId, game, isPlayer) {
    const crowded = game.players.length > 8;
    const seats = game.players.map((userId, i) => gameSeatElement(
        i,
        warGamePlayerInfoElement(userId)
    ));
    const centerChildren = [createElement("div", {
        classList: ["deck-status"],
        children: [
            createElement("p", {classList: ["deck-len"]}),
            trumpCardElement()
        ]
    })];
    if (isPlayer) {
        centerChildren.push(createElement("div", {
            classList: ["deck-actions", "horizontal"],
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
        }));
    } else {
        centerChildren.push(createElement("div", {
            classList: ["deck-actions"]
        }));
    }
    return createElement("div", {
        classList: ["game-table", ...(crowded ? ["crowded-table"] : [])],
        attributes: {seats: game.players.length},
        children: [
            createElement("div", {
                classList: ["table-center"],
                children: centerChildren
            }),
            createElement("div", {classList: ["table-trick"]}),
            ...seats
        ]
    });
}

function gameSeatElement(seatIndex, playerInfoElement) {
    return createElement("div", {
        classList: ["seat"],
        attributes: {seat: seatIndex},
        children: [playerInfoElement]
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

function botBadge() {
    return createElement("span", {
        classList: ["bot-badge"],
        textContent: "BOT",
    });
}

function nameChildren(user) {
    const children = [createElement("span", {classList: ["name-text"], textContent: user.name})];
    if (user.bot) {
        children.push(botBadge());
    }
    return children;
}

function requestNameChange() {
    const currentName = client.user(window.userId).name;
    const name = prompt("Change your name:", currentName);
    if (!name || name === currentName) {
        return;
    }
    document.cookie = "USER_NAME=" + name;
    fetch("./change_name", {method: "POST"});
}

function subscriberNameElement(userId, user) {
    const self = userId === window.userId;
    const element = createElement("li", {
        classList: ["name"],
        attributes: {userId},
        children: self ? [createElement("button", {
            type: "button",
            classList: ["change-name"],
            children: nameChildren(user),
            listeners: {click: requestNameChange}
        })] : nameChildren(user)
    });
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", self);
    return element;
}

function nameElement(userId) {
    const user = client.user(userId);
    const element = createElement("p", {
        attributes: {userId},
        classList: ["name"],
        children: nameChildren(user)
    });
    element.classList.toggle("online", user.online);
    element.classList.toggle("self", userId === window.userId);
    return element;
}

function warGameActionsElement(gameId) {
    return createElement("div", {
        classList: ["horizontal", "war-actions"],
        children: [
            createElement("div", {
                classList: ["horizontal", "war-hand-row"],
                children: [
                    createElement("span", {classList: ["plays-label"], textContent: "Hand: "}),
                    createElement("div", {classList: ["my-war-hand"]})
                ]
            }),
            createElement("button", {
                classList: ["finish-trick"],
                textContent: "Finish Trick",
                listeners: {click: (event) => finishTrick(gameId)}
            })
        ]
    });
}

function warHandCardElement(gameId, card) {
    return createElement("div", {
        classList: ["war-hand-card"],
        attributes: {card: card},
        children: [
            createElement("div", {
                classList: ["war-card"],
                children: [pretty(card)]
            }),
            createElement("div", {
                classList: ["war-card-actions"],
                children: [
                    createElement("button", {
                        classList: ["war-card-action", "play-card"],
                        textContent: "Play",
                        listeners: {click: (event) => playCard(gameId, card)}
                    }),
                    createElement("button", {
                        classList: ["war-card-action", "slough-card"],
                        textContent: "Slough",
                        listeners: {click: (event) => slough(gameId, card)}
                    })
                ]
            })
        ]
    });
}

function rummyGameElement(gameId, game) {
    const element = document.createDocumentFragment();
    element.appendChild(rummyGameTableElement(game));
    if (game.players.includes(window.userId)) {
        element.appendChild(rummyGameActionsElement(gameId));
    }
    return element;
}

function rummyGameTableElement(game) {
    const crowded = game.players.length > 8;
    const seats = game.players.map((userId, i) => gameSeatElement(
        i,
        rummyGamePlayerInfoElement(userId)
    ));
    return createElement("div", {
        classList: ["game-table", ...(crowded ? ["crowded-table"] : [])],
        attributes: {seats: game.players.length},
        children: [
            createElement("div", {
                classList: ["table-center"],
                children: [
                    trumpCardElement(game.phase.trump),
                    createElement("div", {
                        classList: ["deck-actions"]
                    })
                ]
            }),
            createElement("div", {classList: ["table-trick", "rummy-trick"]}),
            ...seats
        ]
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
        classList: ["horizontal", "rummy-actions"],
        children: [
            createElement("span", {classList: ["plays-label"], textContent: "Hand: "}),
            createElement("div", {classList: ["rummy-cards", "horizontal"]}),
            createElement("button", {
                classList: ["play-range"],
                textContent: "Play",
                listeners: {click: (event) => playRun(gameId)}
            }),
            createElement("button", {
                classList: ["pick-up"],
                textContent: "Pick Up",
                listeners: {click: (event) => pickUp(gameId)}
            }),
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
    for (const cardElem of cardsElem.children) {
        cardElem.classList.remove("selected-run");
    }
    if (checkedElems.length == 0) {
        playRangeElem.disabled = true;
        for (const cardElem of cardsElem.children) {
            const checkElem = cardElem.querySelector("input");
            checkElem.disabled = !cardElem.classList.contains("canPlay");
        }
    } else if (checkedElems.length == 1) {
        playRangeElem.disabled = false;
        const checkedCardElem = checkedElems[0].parentElement;
        checkedCardElem.classList.add("selected-run");
        for (const cardElem of cardsElem.children) {
            const checkElem = cardElem.querySelector("input");
            if (cardElem.dataset.card == checkedCardElem.dataset.card) {
                checkElem.disabled = !cardElem.isSameNode(checkedCardElem);
            } else {
                checkElem.disabled = cardElem.dataset.runmin != checkedCardElem.dataset.runmin
                        || !cardElem.classList.contains("canPlay");
            }
        }
    } else if (checkedElems.length == 2) {
        playRangeElem.disabled = false;
        const selectedCardElems = [...checkedElems].map(elem => elem.parentElement);
        const lo = selectedCardElems[0].dataset.card;
        const hi = selectedCardElems[1].dataset.card;
        for (const card of cardsInRange(lo, hi)) {
            const cardElem = selectedCardElems.find(elem => elem.dataset.card == card)
                ?? [...cardsElem.children].find(elem => elem.dataset.card == card);
            cardElem?.classList.add("selected-run");
        }
        for (const cardElem of cardsElem.children) {
            const checkElem = cardElem.querySelector("input");
            checkElem.disabled = !checkElem.checked;
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

function updateTableTrick(trickElem, seatElements, game, trick) {
    for (let i = 0; i < seatElements.length; i++) {
        const next = trick && (trick.winner === undefined
            ? i === trick.next
            : (trick.endMask & (1 << i)) !== 0);
        seatElements[i].classList.toggle("next", !!next);
    }

    trickElem.innerHTML = null;
    if (!trick || trick.plays.length === 0) {
        return;
    }

    for (const play of trick.plays) {
        const userId = game.players[play.player];
        const card = pretty(play.card);
        card.classList.toggle("lead", play.lead);

        const entry = createElement("div", {
            classList: ["trick-card"],
            attributes: {kind: play.kind},
            children: [
                createElement("span", {
                    classList: ["trick-card-who"],
                    textContent: client.user(userId).name
                }),
                card
            ]
        });
        if (play.kind === "slough") {
            entry.title = "Sloughed";
        }
        trickElem.appendChild(entry);
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

function cardsInRange(lo, hi) {
    const ranks = Object.keys(RANKS);
    const start = ranks.indexOf(lo[0]);
    const end = ranks.indexOf(hi[0]);
    return ranks.slice(start, end + 1).map(rank => rank + lo[1]);
}

function trumpCardElement(card) {
    const faceUp = card !== undefined;
    return createElement("div", {
        classList: ["trump-card", faceUp ? "face-up" : "face-down"],
        children: faceUp ? [pretty(card)] : []
    });
}

function pretty(card) {
    return createElement("span", {
        attributes: {suit: card[1]},
        textContent: RANKS[card[0]] + SUITS[card[1]]
    });
}

export function forgetGame(gameId) {
    gameElement(gameId)?.remove();
    gameSummaryElementFor(gameId)?.remove();
    if (selectedGameId === gameId) {
        showGameList(true);
    }
    updateEmptyState();
}

export function updateUser(userId, user) {
    let userNodes = document.querySelectorAll(`[data-userId="${userId}"]`);
    if (userNodes.length == 0) {
        const userNode = subscriberNameElement(userId, user);
        document.getElementById("subscribers").appendChild(userNode);
        userNodes = [userNode];
    }
    for (const userNode of userNodes) {
        userNode.classList.toggle("online", user.online);
        userNode.classList.toggle("self", userId === window.userId);
        const nameText = userNode.querySelector(".name-text");
        if (nameText) {
            nameText.textContent = user.name;
        } else {
            userNode.textContent = user.name;
        }
    }
    for (const userContainerNode of document.querySelectorAll(".sorted-users")) {
        [...userContainerNode.children]
            .filter(child => child.hasAttribute("data-userId"))
            .sort((a, b) => a.textContent.localeCompare(b.textContent)
                || a.getAttribute("data-userId").localeCompare(b.getAttribute("data-userId")))
            .forEach(child => userContainerNode.appendChild(child));
    }
    for (const gameElem of document.getElementById("games").children) {
        const gameId = gameElem.getAttribute("data-gameId");
        const game = client.game(gameId);
        if (game.phase.type === "unstarted") {
            updateUnstartedGame(gameId, game, gameElem);
        }
        updateGameSummary(gameId, game);
    }
}

export function forgetUser(userId) {
    const userNodes = document.querySelectorAll(`[data-userId="${userId}"]`);
    for (const userNode of userNodes) {
        userNode.remove();
    }
}

export function joinGame(gameId, userId) {
    return applyAction(gameId, `{"type":"join","userId":"${userId}"}`);
}

export function leaveGame(gameId, userId) {
    const player = client.game(gameId).players.indexOf(userId);
    return applyAction(gameId, `{"type":"leave","player":${player}}`);
}

export function startGame(gameId, numDecks) {
    return applyAction(gameId, `{"type":"start","numDecks":${numDecks}}`);
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

export function playRun(gameId) {
    const checkedElems = document.querySelectorAll(`[data-gameId="${gameId}"] .rummy-card input:checked`)
    const lo = checkedElems[0].parentElement.dataset.card;
    const hi = checkedElems[checkedElems.length - 1].parentElement.dataset.card;
    disableButtons(gameId);
    applyAction(gameId, `{"type":"playRun","lo":"${lo}","hi":"${hi}"}`);
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
    return fetch(`./apply_action?game_id=${gameId}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: action,
    });
}

async function runSetupAction(button, action, errorMessage, lockSetupUntilUpdate = false) {
    const gameElem = button.closest(".game");
    if (lockSetupUntilUpdate && gameElem) {
        gameElem.setAttribute("data-setup-pending", "");
        for (const setupButton of gameElem.querySelectorAll("button")) {
            setupButton.disabled = true;
        }
    } else {
        button.disabled = true;
    }
    let succeeded = false;
    try {
        const response = await action();
        succeeded = response.ok;
        if (!succeeded) {
            alert(errorMessage);
        }
    } catch {
        alert(errorMessage);
    } finally {
        if (lockSetupUntilUpdate && !succeeded) {
            gameElem?.removeAttribute("data-setup-pending");
        }
        if ((!lockSetupUntilUpdate || !succeeded) && gameElem?.isConnected) {
            button.disabled = false;
            const gameId = gameElem.getAttribute("data-gameId");
            const game = client.game(gameId);
            if (game.phase.type === "unstarted") {
                updateUnstartedGame(gameId, game, gameElem);
            }
        }
    }
}

function signalUpdate() {
    if (document.visibilityState !== "visible") {
        document.title = "* Goat";
    }
}

document.getElementById("new-game").addEventListener("click", async (event) => {
    const button = event.currentTarget;
    button.disabled = true;
    try {
        const response = await fetch("./new_game", {method: "POST"});
        if (!response.ok) {
            alert("A new game could not be created.");
            return;
        }
        const gameId = await response.json();
        showGame(gameId, true);
    } catch {
        alert("A new game could not be created.");
    } finally {
        button.disabled = false;
    }
});

updateEmptyState();

document.getElementById("all-games").addEventListener("click", () => showGameList(true));

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
