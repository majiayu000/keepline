"use strict";
var __defProp = Object.defineProperty;
var __getOwnPropSymbols = Object.getOwnPropertySymbols;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __propIsEnum = Object.prototype.propertyIsEnumerable;
var __defNormalProp = (obj, key, value) => key in obj ? __defProp(obj, key, { enumerable: true, configurable: true, writable: true, value }) : obj[key] = value;
var __spreadValues = (a, b) => {
  for (var prop in b || (b = {}))
    if (__hasOwnProp.call(b, prop))
      __defNormalProp(a, prop, b[prop]);
  if (__getOwnPropSymbols)
    for (var prop of __getOwnPropSymbols(b)) {
      if (__propIsEnum.call(b, prop))
        __defNormalProp(a, prop, b[prop]);
    }
  return a;
};

// src/poker/types.ts
var OpCode = {
  // Client -> Server
  PLAYER_ACTION: 1,
  CHAT_MESSAGE: 2,
  SIT_OUT: 3,
  SIT_IN: 4,
  REQUEST_SEAT: 5,
  // Spectator requests to become a player
  // Server -> Client
  GAME_STATE: 10,
  PLAYER_JOINED: 11,
  PLAYER_LEFT: 12,
  HAND_START: 13,
  HOLE_CARDS: 14,
  COMMUNITY_CARDS: 15,
  PLAYER_TURN: 16,
  PLAYER_ACTED: 17,
  POT_UPDATE: 18,
  SHOWDOWN: 19,
  HAND_RESULT: 20,
  PLAYER_DISCONNECTED: 21,
  PLAYER_RECONNECTED: 22,
  SPECTATOR_JOINED: 23,
  // Spectator joined the match
  SPECTATOR_LEFT: 24,
  // Spectator left the match
  SPECTATOR_TO_PLAYER: 25,
  // Spectator became a player
  SPECTATOR_LIST: 26,
  // List of spectators
  ERROR: 99
};

// src/poker/deck.ts
var SUITS = ["hearts" /* Hearts */, "diamonds" /* Diamonds */, "clubs" /* Clubs */, "spades" /* Spades */];
var RANKS = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
function createDeck() {
  const deck = [];
  for (const suit of SUITS) {
    for (const rank of RANKS) {
      deck.push({ suit, rank });
    }
  }
  return deck;
}
function shuffleDeck(deck) {
  for (let i = deck.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [deck[i], deck[j]] = [deck[j], deck[i]];
  }
  return deck;
}
function dealCards(deck, count) {
  if (count > deck.length) {
    throw new Error(`Cannot deal ${count} cards, only ${deck.length} remaining`);
  }
  return deck.splice(0, count);
}
function dealCard(deck) {
  if (deck.length === 0) {
    throw new Error("Cannot deal from empty deck");
  }
  return deck.shift();
}
function createShuffledDeck() {
  return shuffleDeck(createDeck());
}

// src/poker/game_state.ts
function getActivePlayers(state) {
  return Array.from(Object.values(state.players)).filter((p) => p.status === "active" /* Active */ || p.status === "all_in" /* AllIn */).sort((a, b) => a.seatIndex - b.seatIndex);
}
function getActingPlayers(state) {
  return Array.from(Object.values(state.players)).filter((p) => p.status === "active" /* Active */).sort((a, b) => a.seatIndex - b.seatIndex);
}
function getEligiblePlayers(state) {
  return Array.from(Object.values(state.players)).filter((p) => p.chips > 0 && p.status !== "sitting_out" /* SittingOut */).sort((a, b) => a.seatIndex - b.seatIndex);
}
function getNextPlayerSeat(state, currentSeat, skipFolded = true, skipAllIn = true) {
  const players = Array.from(Object.values(state.players)).sort((a, b) => a.seatIndex - b.seatIndex);
  if (players.length === 0)
    return -1;
  let nextSeat = currentSeat;
  let attempts = 0;
  const maxAttempts = state.maxPlayers;
  do {
    nextSeat = (nextSeat + 1) % state.maxPlayers;
    attempts++;
    const player = players.find((p) => p.seatIndex === nextSeat);
    if (player) {
      if (skipFolded && player.status === "folded" /* Folded */)
        continue;
      if (skipAllIn && player.status === "all_in" /* AllIn */)
        continue;
      if (player.status === "sitting_out" /* SittingOut */)
        continue;
      return nextSeat;
    }
  } while (attempts < maxAttempts);
  return -1;
}
function getPlayerBySeat(state, seatIndex) {
  return Array.from(Object.values(state.players)).find((p) => p.seatIndex === seatIndex);
}
function moveDealerButton(state) {
  const eligiblePlayers = getEligiblePlayers(state);
  if (eligiblePlayers.length === 0)
    return;
  if (state.dealerSeatIndex === -1) {
    state.dealerSeatIndex = eligiblePlayers[0].seatIndex;
  } else {
    state.dealerSeatIndex = getNextPlayerSeat(state, state.dealerSeatIndex, false, false);
  }
}
function assignBlinds(state) {
  const eligiblePlayers = getEligiblePlayers(state);
  Object.values(state.players).forEach((p) => {
    p.isDealer = false;
    p.isSmallBlind = false;
    p.isBigBlind = false;
  });
  const dealer = getPlayerBySeat(state, state.dealerSeatIndex);
  if (dealer)
    dealer.isDealer = true;
  let smallBlindSeat;
  let bigBlindSeat;
  if (eligiblePlayers.length === 2) {
    smallBlindSeat = state.dealerSeatIndex;
    bigBlindSeat = getNextPlayerSeat(state, state.dealerSeatIndex, false, false);
  } else {
    smallBlindSeat = getNextPlayerSeat(state, state.dealerSeatIndex, false, false);
    bigBlindSeat = getNextPlayerSeat(state, smallBlindSeat, false, false);
  }
  const sbPlayer = getPlayerBySeat(state, smallBlindSeat);
  const bbPlayer = getPlayerBySeat(state, bigBlindSeat);
  if (sbPlayer)
    sbPlayer.isSmallBlind = true;
  if (bbPlayer)
    bbPlayer.isBigBlind = true;
  return { smallBlindSeat, bigBlindSeat };
}
function postBlinds(state, smallBlindSeat, bigBlindSeat) {
  const sbPlayer = getPlayerBySeat(state, smallBlindSeat);
  const bbPlayer = getPlayerBySeat(state, bigBlindSeat);
  if (sbPlayer) {
    const sbAmount = Math.min(state.smallBlind, sbPlayer.chips);
    sbPlayer.chips -= sbAmount;
    sbPlayer.currentBet = sbAmount;
    sbPlayer.totalBetThisHand = sbAmount;
    state.pots[0].amount += sbAmount;
  }
  if (bbPlayer) {
    const bbAmount = Math.min(state.bigBlind, bbPlayer.chips);
    bbPlayer.chips -= bbAmount;
    bbPlayer.currentBet = bbAmount;
    bbPlayer.totalBetThisHand = bbAmount;
    state.pots[0].amount += bbAmount;
  }
  state.currentBet = state.bigBlind;
  state.minRaise = state.bigBlind;
  state.lastRaiseAmount = state.bigBlind;
}
function dealHoleCards(state) {
  const holeCards = {};
  const eligiblePlayers = getEligiblePlayers(state);
  for (const player of eligiblePlayers) {
    const cards = dealCards(state.deck, 2);
    player.holeCards = cards;
    player.status = "active" /* Active */;
    holeCards[player.odid] = cards;
  }
  return holeCards;
}
function dealCommunityCards(state, count) {
  dealCard(state.deck);
  const cards = dealCards(state.deck, count);
  state.communityCards.push(...cards);
  return cards;
}
function startBettingRound(state) {
  Object.values(state.players).forEach((p) => {
    p.currentBet = 0;
    p.hasActed = false;
    p.lastAction = void 0;
  });
  state.currentBet = 0;
  state.actionsThisRound = 0;
  if (state.phase === "pre_flop" /* PreFlop */) {
    const eligiblePlayers = getEligiblePlayers(state);
    const bbPlayer = eligiblePlayers.find((p) => p.isBigBlind);
    if (bbPlayer) {
      state.currentPlayerSeatIndex = getNextPlayerSeat(state, bbPlayer.seatIndex);
    }
  } else {
    state.currentPlayerSeatIndex = getNextPlayerSeat(state, state.dealerSeatIndex);
  }
}
function isBettingRoundComplete(state) {
  const actingPlayers = getActingPlayers(state);
  if (actingPlayers.length <= 1)
    return true;
  for (const player of actingPlayers) {
    if (!player.hasActed)
      return false;
    if (player.currentBet !== state.currentBet && player.status !== "all_in" /* AllIn */) {
      return false;
    }
  }
  return true;
}
function isOnlyOnePlayerLeft(state) {
  const activePlayers = getActivePlayers(state);
  return activePlayers.length <= 1;
}
function shouldGoToShowdown(state) {
  const actingPlayers = getActingPlayers(state);
  return actingPlayers.length === 0 && getActivePlayers(state).length > 1;
}
function initNewHand(state, tick) {
  state.handNumber++;
  state.deck = createShuffledDeck();
  state.communityCards = [];
  state.pots = [{ amount: 0, eligiblePlayers: [] }];
  Object.values(state.players).forEach((p) => {
    p.holeCards = [];
    p.currentBet = 0;
    p.totalBetThisHand = 0;
    p.status = p.chips > 0 && p.status !== "sitting_out" /* SittingOut */ ? "waiting" /* Waiting */ : p.status;
    p.hasActed = false;
    p.lastAction = void 0;
    p.isDealer = false;
    p.isSmallBlind = false;
    p.isBigBlind = false;
  });
  state.currentBet = 0;
  state.minRaise = state.bigBlind;
  state.lastRaiseAmount = 0;
  state.actionsThisRound = 0;
  state.turnStartTick = tick;
}
function startNewHand(state, tick) {
  initNewHand(state, tick);
  moveDealerButton(state);
  const { smallBlindSeat, bigBlindSeat } = assignBlinds(state);
  postBlinds(state, smallBlindSeat, bigBlindSeat);
  const eligiblePlayers = getEligiblePlayers(state);
  state.pots[0].eligiblePlayers = eligiblePlayers.map((p) => p.odid);
  const holeCards = dealHoleCards(state);
  state.phase = "pre_flop" /* PreFlop */;
  startBettingRound(state);
  state.currentBet = state.bigBlind;
  return holeCards;
}
function advancePhase(state) {
  switch (state.phase) {
    case "pre_flop" /* PreFlop */:
      state.phase = "flop" /* Flop */;
      dealCommunityCards(state, 3);
      break;
    case "flop" /* Flop */:
      state.phase = "turn" /* Turn */;
      dealCommunityCards(state, 1);
      break;
    case "turn" /* Turn */:
      state.phase = "river" /* River */;
      dealCommunityCards(state, 1);
      break;
    case "river" /* River */:
      state.phase = "showdown" /* Showdown */;
      break;
    default:
      break;
  }
  if (state.phase !== "showdown" /* Showdown */) {
    startBettingRound(state);
  }
  return state.phase;
}
function moveToNextPlayer(state, tick) {
  state.currentPlayerSeatIndex = getNextPlayerSeat(state, state.currentPlayerSeatIndex);
  state.turnStartTick = tick;
  return state.currentPlayerSeatIndex;
}
function hasPlayerTimedOut(state, currentTick) {
  const elapsed = currentTick - state.turnStartTick;
  return elapsed >= state.turnTimeoutTicks;
}
function calculateSidePots(state) {
  const activePlayers = getActivePlayers(state);
  if (activePlayers.length === 0)
    return;
  const allInAmounts = activePlayers.filter((p) => p.status === "all_in" /* AllIn */).map((p) => p.totalBetThisHand).sort((a, b) => a - b);
  if (allInAmounts.length === 0)
    return;
  const pots = [];
  let previousLevel = 0;
  for (const level of [...new Set(allInAmounts)]) {
    const contribution = level - previousLevel;
    const eligiblePlayers = activePlayers.filter((p) => p.totalBetThisHand >= level).map((p) => p.odid);
    let potAmount = 0;
    for (const player of activePlayers) {
      const playerContribution = Math.min(player.totalBetThisHand - previousLevel, contribution);
      if (playerContribution > 0) {
        potAmount += playerContribution;
      }
    }
    if (potAmount > 0) {
      pots.push({ amount: potAmount, eligiblePlayers });
    }
    previousLevel = level;
  }
  const maxAllIn = Math.max(...allInAmounts);
  const remainingPlayers = activePlayers.filter((p) => p.totalBetThisHand > maxAllIn).map((p) => p.odid);
  if (remainingPlayers.length > 0) {
    let remainingAmount = 0;
    for (const player of activePlayers) {
      if (player.totalBetThisHand > maxAllIn) {
        remainingAmount += player.totalBetThisHand - maxAllIn;
      }
    }
    if (remainingAmount > 0) {
      pots.push({ amount: remainingAmount, eligiblePlayers: remainingPlayers });
    }
  }
  if (pots.length > 0) {
    state.pots = pots;
  }
}
function getTotalPot(state) {
  return state.pots.reduce((sum, pot) => sum + pot.amount, 0);
}
function endHandWithSingleWinner(state) {
  const activePlayers = getActivePlayers(state);
  if (activePlayers.length !== 1)
    return null;
  const winner = activePlayers[0];
  const totalPot = getTotalPot(state);
  winner.chips += totalPot;
  state.phase = "waiting" /* Waiting */;
  return { winnerId: winner.odid, amount: totalPot };
}
function getAvailableActions(state, playerId) {
  const player = state.players[playerId];
  if (!player || player.status !== "active" /* Active */) {
    return [];
  }
  const actions = ["fold" /* Fold */];
  const toCall = state.currentBet - player.currentBet;
  if (toCall === 0) {
    actions.push("check" /* Check */);
  } else if (toCall > 0 && player.chips > 0) {
    if (player.chips >= toCall) {
      actions.push("call" /* Call */);
    }
  }
  if (player.chips > toCall) {
    if (state.currentBet === 0) {
      actions.push("bet" /* Bet */);
    } else {
      actions.push("raise" /* Raise */);
    }
  }
  if (player.chips > 0) {
    actions.push("all_in" /* AllIn */);
  }
  return actions;
}
function getBetLimits(state, playerId) {
  const player = state.players[playerId];
  if (!player)
    return { min: 0, max: 0 };
  const toCall = state.currentBet - player.currentBet;
  const minRaise = state.currentBet + state.minRaise;
  const min = Math.max(minRaise - player.currentBet, state.bigBlind);
  const max = player.chips;
  return { min: Math.min(min, max), max };
}

// src/poker/betting.ts
function executePlayerAction(state, playerId, action, amount) {
  const player = state.players[playerId];
  if (!player) {
    return { success: false, error: "Player not found" };
  }
  if (player.seatIndex !== state.currentPlayerSeatIndex) {
    return { success: false, error: "Not your turn" };
  }
  if (player.status !== "active" /* Active */) {
    return { success: false, error: "Player cannot act" };
  }
  const availableActions = getAvailableActions(state, playerId);
  if (!availableActions.includes(action)) {
    return { success: false, error: `Action ${action} not available. Available: ${availableActions.join(", ")}` };
  }
  switch (action) {
    case "fold" /* Fold */:
      return executeFold(state, player);
    case "check" /* Check */:
      return executeCheck(state, player);
    case "call" /* Call */:
      return executeCall(state, player);
    case "bet" /* Bet */:
      return executeBet(state, player, amount);
    case "raise" /* Raise */:
      return executeRaise(state, player, amount);
    case "all_in" /* AllIn */:
      return executeAllIn(state, player);
    default:
      return { success: false, error: "Unknown action" };
  }
}
function executeFold(state, player) {
  player.status = "folded" /* Folded */;
  player.hasActed = true;
  player.lastAction = "fold" /* Fold */;
  state.actionsThisRound++;
  return {
    success: true,
    action: "fold" /* Fold */,
    amount: 0,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: false
  };
}
function executeCheck(state, player) {
  const toCall = state.currentBet - player.currentBet;
  if (toCall > 0) {
    return { success: false, error: "Cannot check, there is a bet to match" };
  }
  player.hasActed = true;
  player.lastAction = "check" /* Check */;
  state.actionsThisRound++;
  return {
    success: true,
    action: "check" /* Check */,
    amount: 0,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: false
  };
}
function executeCall(state, player) {
  const toCall = state.currentBet - player.currentBet;
  if (toCall <= 0) {
    return { success: false, error: "Nothing to call" };
  }
  const actualCall = Math.min(toCall, player.chips);
  const isAllIn = actualCall === player.chips && player.chips < toCall;
  player.chips -= actualCall;
  player.currentBet += actualCall;
  player.totalBetThisHand += actualCall;
  state.pots[0].amount += actualCall;
  player.hasActed = true;
  player.lastAction = "call" /* Call */;
  state.actionsThisRound++;
  if (isAllIn || player.chips === 0) {
    player.status = "all_in" /* AllIn */;
  }
  return {
    success: true,
    action: isAllIn ? "all_in" /* AllIn */ : "call" /* Call */,
    amount: actualCall,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: player.status === "all_in" /* AllIn */
  };
}
function executeBet(state, player, amount) {
  if (state.currentBet > 0) {
    return { success: false, error: "Cannot bet, there is already a bet. Use raise instead." };
  }
  if (amount === void 0) {
    return { success: false, error: "Bet amount required" };
  }
  const { min, max } = getBetLimits(state, player.odid);
  if (amount < min && amount !== player.chips) {
    return { success: false, error: `Minimum bet is ${min}` };
  }
  if (amount > max) {
    return { success: false, error: `Maximum bet is ${max}` };
  }
  const actualBet = Math.min(amount, player.chips);
  player.chips -= actualBet;
  player.currentBet = actualBet;
  player.totalBetThisHand += actualBet;
  state.pots[0].amount += actualBet;
  state.currentBet = actualBet;
  state.lastRaiseAmount = actualBet;
  state.minRaise = actualBet;
  player.hasActed = true;
  player.lastAction = "bet" /* Bet */;
  state.actionsThisRound++;
  Object.values(state.players).forEach((p) => {
    if (p.odid !== player.odid && p.status === "active" /* Active */) {
      p.hasActed = false;
    }
  });
  if (player.chips === 0) {
    player.status = "all_in" /* AllIn */;
  }
  return {
    success: true,
    action: player.chips === 0 ? "all_in" /* AllIn */ : "bet" /* Bet */,
    amount: actualBet,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: player.status === "all_in" /* AllIn */
  };
}
function executeRaise(state, player, amount) {
  if (state.currentBet === 0) {
    return { success: false, error: "Cannot raise, no bet to raise. Use bet instead." };
  }
  if (amount === void 0) {
    return { success: false, error: "Raise amount required" };
  }
  const toCall = state.currentBet - player.currentBet;
  const { min, max } = getBetLimits(state, player.odid);
  const raiseAmount = amount - state.currentBet;
  if (raiseAmount < state.minRaise && amount !== player.chips + player.currentBet) {
    return { success: false, error: `Minimum raise is ${state.minRaise}. Total bet must be at least ${state.currentBet + state.minRaise}` };
  }
  const actualAmount = Math.min(amount - player.currentBet, player.chips);
  if (actualAmount <= 0) {
    return { success: false, error: "Invalid raise amount" };
  }
  player.chips -= actualAmount;
  player.currentBet += actualAmount;
  player.totalBetThisHand += actualAmount;
  state.pots[0].amount += actualAmount;
  const newCurrentBet = player.currentBet;
  state.lastRaiseAmount = newCurrentBet - state.currentBet;
  state.minRaise = state.lastRaiseAmount;
  state.currentBet = newCurrentBet;
  player.hasActed = true;
  player.lastAction = "raise" /* Raise */;
  state.actionsThisRound++;
  Object.values(state.players).forEach((p) => {
    if (p.odid !== player.odid && p.status === "active" /* Active */) {
      p.hasActed = false;
    }
  });
  if (player.chips === 0) {
    player.status = "all_in" /* AllIn */;
  }
  return {
    success: true,
    action: player.chips === 0 ? "all_in" /* AllIn */ : "raise" /* Raise */,
    amount: actualAmount,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: player.status === "all_in" /* AllIn */
  };
}
function executeAllIn(state, player) {
  if (player.chips <= 0) {
    return { success: false, error: "No chips to go all-in" };
  }
  const allInAmount = player.chips;
  const newBet = player.currentBet + allInAmount;
  player.chips = 0;
  player.currentBet = newBet;
  player.totalBetThisHand += allInAmount;
  state.pots[0].amount += allInAmount;
  player.hasActed = true;
  player.lastAction = "all_in" /* AllIn */;
  player.status = "all_in" /* AllIn */;
  state.actionsThisRound++;
  if (newBet > state.currentBet) {
    const raiseAmount = newBet - state.currentBet;
    state.lastRaiseAmount = raiseAmount;
    state.minRaise = Math.max(state.minRaise, raiseAmount);
    state.currentBet = newBet;
    Object.values(state.players).forEach((p) => {
      if (p.odid !== player.odid && p.status === "active" /* Active */) {
        p.hasActed = false;
      }
    });
  }
  return {
    success: true,
    action: "all_in" /* AllIn */,
    amount: allInAmount,
    newChips: 0,
    newPotTotal: getTotalPot(state),
    playerAllIn: true
  };
}
function autoFold(state, playerId) {
  const player = state.players[playerId];
  if (!player) {
    return { success: false, error: "Player not found" };
  }
  return executeFold(state, player);
}
function getCallAmount(state, playerId) {
  const player = state.players[playerId];
  if (!player)
    return 0;
  return Math.min(state.currentBet - player.currentBet, player.chips);
}
function getActionInfo(state, playerId) {
  const player = state.players[playerId];
  const availableActions = getAvailableActions(state, playerId);
  const { min, max } = getBetLimits(state, playerId);
  return {
    canCheck: availableActions.includes("check" /* Check */),
    canCall: availableActions.includes("call" /* Call */),
    canBet: availableActions.includes("bet" /* Bet */),
    canRaise: availableActions.includes("raise" /* Raise */),
    canAllIn: availableActions.includes("all_in" /* AllIn */),
    callAmount: player ? getCallAmount(state, playerId) : 0,
    minBet: min,
    maxBet: max,
    potTotal: getTotalPot(state)
  };
}

// src/poker/hand_evaluator.ts
function combinations(arr, k) {
  if (k === 0)
    return [[]];
  if (arr.length < k)
    return [];
  const result = [];
  const first = arr[0];
  const rest = arr.slice(1);
  for (const combo of combinations(rest, k - 1)) {
    result.push([first, ...combo]);
  }
  result.push(...combinations(rest, k));
  return result;
}
function sortByRank(cards) {
  return [...cards].sort((a, b) => b.rank - a.rank);
}
function groupByRank(cards) {
  const groups = /* @__PURE__ */ new Map();
  for (const card of cards) {
    const existing = groups.get(card.rank) || [];
    existing.push(card);
    groups.set(card.rank, existing);
  }
  return groups;
}
function checkStraight(cards) {
  if (cards.length < 5)
    return null;
  const ranks = [...new Set(cards.map((c) => c.rank))].sort((a, b) => b - a);
  if (ranks.length < 5)
    return null;
  for (let i = 0; i <= ranks.length - 5; i++) {
    if (ranks[i] - ranks[i + 4] === 4) {
      return ranks[i];
    }
  }
  if (ranks.includes(14) && ranks.includes(2) && ranks.includes(3) && ranks.includes(4) && ranks.includes(5)) {
    return 5;
  }
  return null;
}
function getRankName(rank) {
  switch (rank) {
    case 14:
      return "Ace";
    case 13:
      return "King";
    case 12:
      return "Queen";
    case 11:
      return "Jack";
    default:
      return rank.toString();
  }
}
function getRankNamePlural(rank) {
  switch (rank) {
    case 14:
      return "Aces";
    case 13:
      return "Kings";
    case 12:
      return "Queens";
    case 11:
      return "Jacks";
    case 6:
      return "Sixes";
    default:
      return rank.toString() + "s";
  }
}
function evaluate5Cards(cards) {
  if (cards.length !== 5) {
    throw new Error("Must evaluate exactly 5 cards");
  }
  const sorted = sortByRank(cards);
  const byRank = groupByRank(cards);
  const rankCounts = Array.from(byRank.entries()).map(([rank, cs]) => ({ rank, count: cs.length })).sort((a, b) => b.count - a.count || b.rank - a.rank);
  const isFlush = new Set(cards.map((c) => c.suit)).size === 1;
  const straightHigh = checkStraight(cards);
  const isStraight = straightHigh !== null;
  if (isFlush && isStraight && straightHigh === 14) {
    return {
      rank: 10 /* RoyalFlush */,
      cards: sorted,
      kickers: [],
      description: "Royal Flush"
    };
  }
  if (isFlush && isStraight) {
    return {
      rank: 9 /* StraightFlush */,
      cards: sorted,
      kickers: [straightHigh],
      description: `Straight Flush, ${getRankName(straightHigh)} high`
    };
  }
  if (rankCounts[0].count === 4) {
    const quadRank = rankCounts[0].rank;
    const kicker = rankCounts[1].rank;
    return {
      rank: 8 /* FourOfAKind */,
      cards: sorted,
      kickers: [quadRank, kicker],
      description: `Four ${getRankNamePlural(quadRank)}`
    };
  }
  if (rankCounts[0].count === 3 && rankCounts[1].count === 2) {
    const tripsRank = rankCounts[0].rank;
    const pairRank = rankCounts[1].rank;
    return {
      rank: 7 /* FullHouse */,
      cards: sorted,
      kickers: [tripsRank, pairRank],
      description: `Full House, ${getRankNamePlural(tripsRank)} full of ${getRankNamePlural(pairRank)}`
    };
  }
  if (isFlush) {
    return {
      rank: 6 /* Flush */,
      cards: sorted,
      kickers: sorted.map((c) => c.rank),
      description: `Flush, ${getRankName(sorted[0].rank)} high`
    };
  }
  if (isStraight) {
    return {
      rank: 5 /* Straight */,
      cards: sorted,
      kickers: [straightHigh],
      description: `Straight, ${getRankName(straightHigh)} high`
    };
  }
  if (rankCounts[0].count === 3) {
    const tripsRank = rankCounts[0].rank;
    const kickers = rankCounts.slice(1).map((rc) => rc.rank);
    return {
      rank: 4 /* ThreeOfAKind */,
      cards: sorted,
      kickers: [tripsRank, ...kickers],
      description: `Three ${getRankNamePlural(tripsRank)}`
    };
  }
  if (rankCounts[0].count === 2 && rankCounts[1].count === 2) {
    const highPair = Math.max(rankCounts[0].rank, rankCounts[1].rank);
    const lowPair = Math.min(rankCounts[0].rank, rankCounts[1].rank);
    const kicker = rankCounts[2].rank;
    return {
      rank: 3 /* TwoPair */,
      cards: sorted,
      kickers: [highPair, lowPair, kicker],
      description: `Two Pair, ${getRankNamePlural(highPair)} and ${getRankNamePlural(lowPair)}`
    };
  }
  if (rankCounts[0].count === 2) {
    const pairRank = rankCounts[0].rank;
    const kickers = rankCounts.slice(1).map((rc) => rc.rank);
    return {
      rank: 2 /* OnePair */,
      cards: sorted,
      kickers: [pairRank, ...kickers],
      description: `Pair of ${getRankNamePlural(pairRank)}`
    };
  }
  return {
    rank: 1 /* HighCard */,
    cards: sorted,
    kickers: sorted.map((c) => c.rank),
    description: `High Card ${getRankName(sorted[0].rank)}`
  };
}
function evaluateHand(cards) {
  if (cards.length < 5) {
    throw new Error("Need at least 5 cards to evaluate");
  }
  if (cards.length === 5) {
    return evaluate5Cards(cards);
  }
  const allCombinations = combinations(cards, 5);
  let best = null;
  for (const combo of allCombinations) {
    const evaluated = evaluate5Cards(combo);
    if (best === null || compareEvaluatedHands(evaluated, best) > 0) {
      best = evaluated;
    }
  }
  return best;
}
function compareEvaluatedHands(hand1, hand2) {
  var _a, _b;
  if (hand1.rank > hand2.rank)
    return 1;
  if (hand1.rank < hand2.rank)
    return -1;
  const maxKickers = Math.max(hand1.kickers.length, hand2.kickers.length);
  for (let i = 0; i < maxKickers; i++) {
    const k1 = (_a = hand1.kickers[i]) != null ? _a : 0;
    const k2 = (_b = hand2.kickers[i]) != null ? _b : 0;
    if (k1 > k2)
      return 1;
    if (k1 < k2)
      return -1;
  }
  return 0;
}

// src/poker/match_handler.ts
var TICK_RATE = 10;
var TURN_TIMEOUT_SECONDS = 30;
var DISCONNECT_GRACE_SECONDS = 60;
var DEFAULT_SETTINGS = {
  minPlayers: 2,
  maxPlayers: 9,
  smallBlind: 10,
  bigBlind: 20,
  startingChips: 1e3,
  maxSpectators: 50
  // Maximum spectators per match
};
function findAvailableSeat(players, maxPlayers) {
  const takenSeats = /* @__PURE__ */ new Set();
  Object.values(players).forEach((p) => takenSeats.add(p.seatIndex));
  for (let i = 0; i < maxPlayers; i++) {
    if (!takenSeats.has(i)) {
      return i;
    }
  }
  return -1;
}
function broadcastMessage(dispatcher, opCode, data, presences, sender) {
  const payload = JSON.stringify(data);
  dispatcher.broadcastMessage(opCode, payload, presences, sender, true);
}
function sendMessage(dispatcher, opCode, data, presence) {
  const payload = JSON.stringify(data);
  dispatcher.broadcastMessage(opCode, payload, [presence], null, true);
}
function getPublicPlayerInfo(player) {
  return {
    odid: player.odid,
    displayName: player.odisplayName,
    seatIndex: player.seatIndex,
    chips: player.chips,
    status: player.status,
    currentBet: player.currentBet,
    isDealer: player.isDealer,
    lastAction: player.lastAction,
    isConnected: player.isConnected
  };
}
function getPublicGameState(state) {
  const players = [];
  Object.values(state.players).forEach((p) => players.push(getPublicPlayerInfo(p)));
  return {
    phase: state.phase,
    communityCards: state.communityCards,
    pots: state.pots,
    currentBet: state.currentBet,
    currentPlayerSeat: state.currentPlayerSeatIndex,
    dealerSeat: state.dealerSeatIndex,
    handNumber: state.handNumber,
    players,
    spectatorCount: Object.keys(state.spectators).length,
    spectators: getSpectatorList(state)
  };
}
var matchInit = function(ctx, logger, nk, params) {
  logger.info("Initializing poker match", { params });
  const label = params.label || "Texas Hold'em";
  const state = {
    matchId: ctx.matchId || "",
    tickRate: TICK_RATE,
    label,
    minPlayers: params.minPlayers ? parseInt(params.minPlayers) : DEFAULT_SETTINGS.minPlayers,
    maxPlayers: params.maxPlayers ? parseInt(params.maxPlayers) : DEFAULT_SETTINGS.maxPlayers,
    smallBlind: params.smallBlind ? parseInt(params.smallBlind) : DEFAULT_SETTINGS.smallBlind,
    bigBlind: params.bigBlind ? parseInt(params.bigBlind) : DEFAULT_SETTINGS.bigBlind,
    startingChips: params.startingChips ? parseInt(params.startingChips) : DEFAULT_SETTINGS.startingChips,
    maxSpectators: params.maxSpectators ? parseInt(params.maxSpectators) : DEFAULT_SETTINGS.maxSpectators,
    phase: "waiting" /* Waiting */,
    players: {},
    spectators: {},
    deck: [],
    communityCards: [],
    pots: [{ amount: 0, eligiblePlayers: [] }],
    dealerSeatIndex: -1,
    currentPlayerSeatIndex: -1,
    currentBet: 0,
    minRaise: 0,
    lastRaiseAmount: 0,
    turnStartTick: 0,
    turnTimeoutTicks: TURN_TIMEOUT_SECONDS * TICK_RATE,
    handNumber: 0,
    actionsThisRound: 0
  };
  return {
    state,
    tickRate: TICK_RATE,
    label: JSON.stringify({
      name: label,
      players: 0,
      maxPlayers: state.maxPlayers,
      blinds: `${state.smallBlind}/${state.bigBlind}`
    })
  };
};
var matchJoinAttempt = function(ctx, logger, nk, dispatcher, tick, state, presence, metadata) {
  const asSpectator = (metadata == null ? void 0 : metadata.spectator) === true;
  logger.info("Join attempt", { userId: presence.userId, username: presence.username, asSpectator });
  if (presence.userId in state.players) {
    return { state, accept: true };
  }
  if (presence.userId in state.spectators) {
    return { state, accept: true };
  }
  if (asSpectator) {
    if (Object.keys(state.spectators).length >= state.maxSpectators) {
      return { state, accept: false, rejectMessage: "Spectator limit reached" };
    }
    return { state, accept: true };
  }
  if (Object.keys(state.players).length >= state.maxPlayers) {
    return { state, accept: false, rejectMessage: "Match is full. Try joining as spectator." };
  }
  return { state, accept: true };
};
var matchJoin = function(ctx, logger, nk, dispatcher, tick, state, presences) {
  for (const presence of presences) {
    logger.info("User joined", { userId: presence.userId, username: presence.username });
    if (presence.userId in state.players) {
      const player2 = state.players[presence.userId];
      const wasDisconnected = !player2.isConnected;
      player2.isConnected = true;
      player2.disconnectedAt = void 0;
      if (player2.status === "sitting_out" /* SittingOut */ && state.phase === "waiting" /* Waiting */ && player2.chips > 0) {
        player2.status = "waiting" /* Waiting */;
      }
      logger.info("Player rejoined", {
        userId: presence.userId,
        seatIndex: player2.seatIndex,
        wasDisconnected,
        status: player2.status
      });
      if (wasDisconnected) {
        broadcastMessage(dispatcher, OpCode.PLAYER_RECONNECTED, {
          odid: presence.userId,
          displayName: player2.odisplayName,
          seatIndex: player2.seatIndex
        });
      }
      sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);
      if (player2.holeCards.length > 0 && player2.status !== "folded" /* Folded */) {
        sendMessage(dispatcher, OpCode.HOLE_CARDS, { cards: player2.holeCards }, presence);
      }
      if (state.currentPlayerSeatIndex === player2.seatIndex) {
        notifyCurrentPlayer(state, dispatcher);
      }
      continue;
    }
    if (presence.userId in state.spectators) {
      logger.info("Spectator rejoined", { userId: presence.userId });
      sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);
      sendMessage(dispatcher, OpCode.SPECTATOR_LIST, {
        spectators: getSpectatorList(state)
      }, presence);
      continue;
    }
    const seatIndex = findAvailableSeat(state.players, state.maxPlayers);
    if (seatIndex === -1) {
      const spectator = {
        odid: presence.userId,
        displayName: presence.username || `Spectator`,
        joinedAt: tick
      };
      state.spectators[presence.userId] = spectator;
      logger.info("Spectator added", { userId: presence.userId });
      broadcastMessage(dispatcher, OpCode.SPECTATOR_JOINED, {
        odid: presence.userId,
        displayName: spectator.displayName
      });
      sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);
      sendMessage(dispatcher, OpCode.SPECTATOR_LIST, {
        spectators: getSpectatorList(state)
      }, presence);
      continue;
    }
    const player = {
      odid: presence.userId,
      odisplayName: presence.username || `Player ${seatIndex + 1}`,
      seatIndex,
      chips: state.startingChips,
      status: "waiting" /* Waiting */,
      holeCards: [],
      currentBet: 0,
      totalBetThisHand: 0,
      isDealer: false,
      isSmallBlind: false,
      isBigBlind: false,
      hasActed: false,
      isConnected: true
    };
    state.players[presence.userId] = player;
    logger.info("Player added", { userId: presence.userId, seatIndex, chips: player.chips });
    broadcastMessage(dispatcher, OpCode.PLAYER_JOINED, getPublicPlayerInfo(player));
    sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);
    if (Object.keys(state.spectators).length > 0) {
      sendMessage(dispatcher, OpCode.SPECTATOR_LIST, {
        spectators: getSpectatorList(state)
      }, presence);
    }
  }
  updateMatchLabel(state, dispatcher);
  return { state };
};
function getSpectatorList(state) {
  const spectators = [];
  Object.values(state.spectators).forEach((s) => {
    spectators.push({ odid: s.odid, displayName: s.displayName });
  });
  return spectators;
}
function updateMatchLabel(state, dispatcher) {
  let connectedPlayers = 0;
  Object.values(state.players).forEach((p) => {
    if (p.isConnected)
      connectedPlayers++;
  });
  const labelData = {
    name: state.label,
    players: connectedPlayers,
    maxPlayers: state.maxPlayers,
    spectators: Object.keys(state.spectators).length,
    blinds: `${state.smallBlind}/${state.bigBlind}`,
    phase: state.phase
  };
  dispatcher.matchLabelUpdate(JSON.stringify(labelData));
}
var matchLeave = function(ctx, logger, nk, dispatcher, tick, state, presences) {
  for (const presence of presences) {
    logger.info("User disconnected", { userId: presence.userId });
    if (presence.userId in state.spectators) {
      const spectator = state.spectators[presence.userId];
      delete state.spectators[presence.userId];
      logger.info("Spectator removed", { userId: presence.userId });
      broadcastMessage(dispatcher, OpCode.SPECTATOR_LEFT, {
        odid: presence.userId,
        displayName: spectator.displayName
      });
      continue;
    }
    const player = state.players[presence.userId];
    if (!player)
      continue;
    player.isConnected = false;
    player.disconnectedAt = tick;
    if (state.phase !== "waiting" /* Waiting */) {
      logger.info("Player disconnected during game, starting grace period", {
        userId: presence.userId,
        phase: state.phase,
        isCurrentPlayer: state.currentPlayerSeatIndex === player.seatIndex
      });
      broadcastMessage(dispatcher, OpCode.PLAYER_DISCONNECTED, {
        odid: presence.userId,
        displayName: player.odisplayName,
        seatIndex: player.seatIndex,
        graceSeconds: DISCONNECT_GRACE_SECONDS
      });
    } else {
      delete state.players[presence.userId];
      logger.info("Player removed (game not started)", { userId: presence.userId });
      broadcastMessage(dispatcher, OpCode.PLAYER_LEFT, {
        odid: presence.userId,
        sittingOut: false
      });
    }
  }
  updateMatchLabel(state, dispatcher);
  if (Object.keys(state.players).length === 0 && Object.keys(state.spectators).length === 0) {
    logger.info("No players or spectators left, ending match");
    return null;
  }
  return { state };
};
function hasDisconnectedPlayerTimedOut(player, tick) {
  if (player.isConnected || !player.disconnectedAt)
    return false;
  const elapsedTicks = tick - player.disconnectedAt;
  const graceTicks = DISCONNECT_GRACE_SECONDS * TICK_RATE;
  return elapsedTicks >= graceTicks;
}
function handleDisconnectedPlayers(state, dispatcher, logger, tick) {
  let stateChanged = false;
  Object.entries(state.players).forEach(([odid, player]) => {
    if (hasDisconnectedPlayerTimedOut(player, tick)) {
      logger.info("Disconnected player grace period expired", { userId: odid });
      if ((player.status === "active" /* Active */ || player.status === "all_in" /* AllIn */) && state.currentPlayerSeatIndex === player.seatIndex) {
        const result = autoFold(state, odid);
        if (result.success) {
          broadcastMessage(dispatcher, OpCode.PLAYER_ACTED, {
            odid,
            action: "fold" /* Fold */,
            amount: 0,
            newChips: player.chips,
            potTotal: getTotalPot(state),
            timeout: true,
            disconnected: true
          });
          stateChanged = true;
        }
      }
      player.status = "sitting_out" /* SittingOut */;
      player.disconnectedAt = void 0;
      broadcastMessage(dispatcher, OpCode.PLAYER_LEFT, {
        odid,
        sittingOut: true,
        reason: "disconnect_timeout"
      });
    }
  });
  return stateChanged;
}
function handleRequestSeat(state, dispatcher, logger, tick, presence) {
  const odid = presence.userId;
  const spectator = state.spectators[odid];
  if (!spectator) {
    sendMessage(dispatcher, OpCode.ERROR, { message: "You are not a spectator" }, presence);
    return;
  }
  const seatIndex = findAvailableSeat(state.players, state.maxPlayers);
  if (seatIndex === -1) {
    sendMessage(dispatcher, OpCode.ERROR, { message: "No available seats" }, presence);
    return;
  }
  delete state.spectators[odid];
  const player = {
    odid,
    odisplayName: spectator.displayName,
    seatIndex,
    chips: state.startingChips,
    status: "waiting" /* Waiting */,
    holeCards: [],
    currentBet: 0,
    totalBetThisHand: 0,
    isDealer: false,
    isSmallBlind: false,
    isBigBlind: false,
    hasActed: false,
    isConnected: true
  };
  state.players[odid] = player;
  logger.info("Spectator became player", { userId: odid, seatIndex, chips: player.chips });
  broadcastMessage(dispatcher, OpCode.SPECTATOR_LEFT, {
    odid,
    displayName: spectator.displayName
  });
  broadcastMessage(dispatcher, OpCode.PLAYER_JOINED, getPublicPlayerInfo(player));
  broadcastMessage(dispatcher, OpCode.SPECTATOR_TO_PLAYER, {
    odid,
    seatIndex,
    chips: player.chips
  });
  sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);
  updateMatchLabel(state, dispatcher);
}
var matchLoop = function(ctx, logger, nk, dispatcher, tick, state, messages) {
  const disconnectHandled = handleDisconnectedPlayers(state, dispatcher, logger, tick);
  if (disconnectHandled) {
    handlePostAction(state, dispatcher, logger, tick);
  }
  for (const message of messages) {
    if (message.opCode === OpCode.PLAYER_ACTION) {
      try {
        const data = JSON.parse(nk.binaryToString(message.data));
        const result = executePlayerAction(state, message.sender.userId, data.action, data.amount);
        if (result.success) {
          logger.info("Player action executed", {
            userId: message.sender.userId,
            action: result.action,
            amount: result.amount
          });
          broadcastMessage(dispatcher, OpCode.PLAYER_ACTED, {
            odid: message.sender.userId,
            action: result.action,
            amount: result.amount,
            newChips: result.newChips,
            potTotal: result.newPotTotal
          });
          handlePostAction(state, dispatcher, logger, tick);
        } else {
          sendMessage(dispatcher, OpCode.ERROR, { message: result.error }, message.sender);
          logger.warn("Invalid player action", {
            userId: message.sender.userId,
            action: data.action,
            error: result.error
          });
        }
      } catch (e) {
        logger.error("Failed to parse player action", { error: e });
        sendMessage(dispatcher, OpCode.ERROR, { message: "Invalid action format" }, message.sender);
      }
    } else if (message.opCode === OpCode.CHAT_MESSAGE) {
      try {
        const chatData = JSON.parse(nk.binaryToString(message.data));
        broadcastMessage(dispatcher, OpCode.CHAT_MESSAGE, {
          odid: message.sender.userId,
          username: message.sender.username,
          message: chatData.message
        });
      } catch (e) {
        logger.error("Failed to parse chat message", { error: e });
      }
    } else if (message.opCode === OpCode.REQUEST_SEAT) {
      handleRequestSeat(state, dispatcher, logger, tick, message.sender);
    }
  }
  switch (state.phase) {
    case "waiting" /* Waiting */:
      const eligiblePlayers = getEligiblePlayers(state);
      if (eligiblePlayers.length >= state.minPlayers) {
        logger.info("Starting new hand", { playerCount: eligiblePlayers.length });
        const holeCards = startNewHand(state, tick);
        broadcastMessage(dispatcher, OpCode.HAND_START, {
          handNumber: state.handNumber,
          dealerSeat: state.dealerSeatIndex,
          smallBlind: state.smallBlind,
          bigBlind: state.bigBlind
        });
        Object.entries(holeCards).forEach(([odid, cards]) => {
          const presence = getPresenceByUserId(ctx, odid);
          if (presence) {
            sendMessage(dispatcher, OpCode.HOLE_CARDS, { cards }, presence);
          }
        });
        broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));
        notifyCurrentPlayer(state, dispatcher);
      }
      break;
    case "pre_flop" /* PreFlop */:
    case "flop" /* Flop */:
    case "turn" /* Turn */:
    case "river" /* River */:
      if (hasPlayerTimedOut(state, tick)) {
        const currentPlayer = getPlayerBySeat(state, state.currentPlayerSeatIndex);
        if (currentPlayer) {
          logger.info("Player timed out, auto-folding", { userId: currentPlayer.odid });
          const result = autoFold(state, currentPlayer.odid);
          if (result.success) {
            broadcastMessage(dispatcher, OpCode.PLAYER_ACTED, {
              odid: currentPlayer.odid,
              action: "fold" /* Fold */,
              amount: 0,
              newChips: currentPlayer.chips,
              potTotal: getTotalPot(state),
              timeout: true
            });
            handlePostAction(state, dispatcher, logger, tick);
          }
        }
      }
      break;
    case "showdown" /* Showdown */:
      const showdownResult = executeShowdown(state, logger);
      broadcastMessage(dispatcher, OpCode.SHOWDOWN, {
        players: showdownResult.showdownPlayers
      });
      broadcastMessage(dispatcher, OpCode.HAND_RESULT, {
        winners: showdownResult.winners,
        showdown: showdownResult.showdownPlayers
      });
      state.phase = "waiting" /* Waiting */;
      broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));
      break;
  }
  return { state };
};
function executeShowdown(state, logger) {
  logger.info("Executing showdown");
  const showdownPlayers = [];
  Object.values(state.players).forEach((player) => {
    if (player.status === "active" /* Active */ || player.status === "all_in" /* AllIn */) {
      showdownPlayers.push(player);
    }
  });
  logger.info("Showdown players", { count: showdownPlayers.length });
  const evaluatedPlayers = [];
  for (const player of showdownPlayers) {
    const allCards = [...player.holeCards, ...state.communityCards];
    try {
      const evaluation = evaluateHand(allCards);
      evaluatedPlayers.push({ player, evaluation });
      logger.info("Evaluated player hand", {
        odid: player.odid,
        holeCards: player.holeCards,
        rank: evaluation.rank,
        description: evaluation.description
      });
    } catch (e) {
      logger.error("Failed to evaluate hand", { odid: player.odid, error: e });
    }
  }
  if (state.pots.length <= 1 || state.pots[0].eligiblePlayers.length === 0) {
    calculateSidePots(state);
  }
  const winners = [];
  const potWinners = {};
  for (const pot of state.pots) {
    if (pot.amount === 0)
      continue;
    const eligibleEvaluated = evaluatedPlayers.filter(
      (ep) => pot.eligiblePlayers.includes(ep.player.odid)
    );
    if (eligibleEvaluated.length === 0) {
      logger.warn("No eligible players for pot", { pot });
      continue;
    }
    let best = eligibleEvaluated[0];
    const potWinnersList = [best];
    for (let i = 1; i < eligibleEvaluated.length; i++) {
      const comparison = compareEvaluatedHands(eligibleEvaluated[i].evaluation, best.evaluation);
      if (comparison > 0) {
        best = eligibleEvaluated[i];
        potWinnersList.length = 0;
        potWinnersList.push(best);
      } else if (comparison === 0) {
        potWinnersList.push(eligibleEvaluated[i]);
      }
    }
    const shareAmount = Math.floor(pot.amount / potWinnersList.length);
    const remainder = pot.amount % potWinnersList.length;
    for (let i = 0; i < potWinnersList.length; i++) {
      const winner = potWinnersList[i];
      const winAmount = shareAmount + (i === 0 ? remainder : 0);
      winner.player.chips += winAmount;
      const currentWinnings = potWinners[winner.player.odid] || 0;
      potWinners[winner.player.odid] = currentWinnings + winAmount;
      logger.info("Pot distributed", {
        odid: winner.player.odid,
        amount: winAmount,
        handRank: winner.evaluation.rank,
        description: winner.evaluation.description
      });
    }
  }
  Object.entries(potWinners).forEach(([odid, amount]) => {
    const ep = evaluatedPlayers.find((e) => e.player.odid === odid);
    if (ep) {
      winners.push({
        odid,
        amount,
        hand: ep.evaluation.cards,
        handRank: ep.evaluation.rank,
        handDescription: ep.evaluation.description
      });
    }
  });
  const showdownPlayersInfo = evaluatedPlayers.map((ep) => ({
    odid: ep.player.odid,
    cards: ep.player.holeCards,
    handRank: ep.evaluation.rank,
    handDescription: ep.evaluation.description,
    bestHand: ep.evaluation.cards
  }));
  state.pots = [{ amount: 0, eligiblePlayers: [] }];
  Object.values(state.players).forEach((player) => {
    player.holeCards = [];
    player.currentBet = 0;
    player.totalBetThisHand = 0;
    player.hasActed = false;
    player.lastAction = void 0;
    player.isDealer = false;
    player.isSmallBlind = false;
    player.isBigBlind = false;
    if (player.chips > 0 && player.status !== "sitting_out" /* SittingOut */) {
      player.status = "waiting" /* Waiting */;
    } else if (player.chips === 0) {
      player.status = "sitting_out" /* SittingOut */;
    }
  });
  state.communityCards = [];
  state.deck = [];
  state.currentBet = 0;
  state.minRaise = 0;
  state.lastRaiseAmount = 0;
  logger.info("Showdown complete", { winners: winners.map((w) => ({ odid: w.odid, amount: w.amount })) });
  return {
    winners,
    showdownPlayers: showdownPlayersInfo
  };
}
function handlePostAction(state, dispatcher, logger, tick) {
  if (isOnlyOnePlayerLeft(state)) {
    logger.info("Only one player left, ending hand");
    const result = endHandWithSingleWinner(state);
    if (result) {
      broadcastMessage(dispatcher, OpCode.HAND_RESULT, {
        winners: [{
          odid: result.winnerId,
          amount: result.amount
        }],
        showdown: []
      });
      broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));
    }
    return;
  }
  if (isBettingRoundComplete(state)) {
    logger.info("Betting round complete", { phase: state.phase });
    if (shouldGoToShowdown(state)) {
      logger.info("All players all-in, going to showdown");
      calculateSidePots(state);
      while (state.phase !== "showdown" /* Showdown */) {
        advancePhase(state);
        broadcastMessage(dispatcher, OpCode.COMMUNITY_CARDS, {
          cards: state.communityCards,
          phase: state.phase
        });
      }
      broadcastMessage(dispatcher, OpCode.POT_UPDATE, {
        pots: state.pots,
        total: getTotalPot(state)
      });
      return;
    }
    const newPhase = advancePhase(state);
    logger.info("Advanced to phase", { phase: newPhase });
    if (newPhase !== "showdown" /* Showdown */) {
      broadcastMessage(dispatcher, OpCode.COMMUNITY_CARDS, {
        cards: state.communityCards,
        phase: newPhase
      });
      broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));
      notifyCurrentPlayer(state, dispatcher);
    }
    return;
  }
  moveToNextPlayer(state, tick);
  broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));
  notifyCurrentPlayer(state, dispatcher);
}
function notifyCurrentPlayer(state, dispatcher) {
  const currentPlayer = getPlayerBySeat(state, state.currentPlayerSeatIndex);
  if (!currentPlayer)
    return;
  const actionInfo = getActionInfo(state, currentPlayer.odid);
  broadcastMessage(dispatcher, OpCode.PLAYER_TURN, __spreadValues({
    odid: currentPlayer.odid,
    seatIndex: currentPlayer.seatIndex,
    timeoutSeconds: state.turnTimeoutTicks / TICK_RATE
  }, actionInfo));
}
function getPresenceByUserId(ctx, odid) {
  return {
    userId: odid,
    sessionId: "",
    username: "",
    node: ""
  };
}
var matchTerminate = function(ctx, logger, nk, dispatcher, tick, state, graceSeconds) {
  logger.info("Match terminating", {
    matchId: state.matchId,
    graceSeconds,
    playerCount: Object.keys(state.players).length
  });
  return { state };
};
var matchSignal = function(ctx, logger, nk, dispatcher, tick, state, data) {
  logger.info("Match signal received", { data });
  return { state, data: "signal_received" };
};
var pokerMatchHandler = {
  matchInit: matchInit,
  matchJoinAttempt: matchJoinAttempt,
  matchJoin: matchJoin,
  matchLeave: matchLeave,
  matchLoop: matchLoop,
  matchTerminate: matchTerminate,
  matchSignal: matchSignal
};

// src/rpc/find_match.ts
var POKER_MATCH_MODULE = "poker";
var findMatchRpc = function(ctx, logger, nk, payload) {
  logger.info("Find match RPC called", { userId: ctx.userId, payload });
  let request = {};
  if (payload && payload.length > 0) {
    try {
      request = JSON.parse(payload);
    } catch (e) {
      logger.error("Failed to parse payload", { error: e });
    }
  }
  const limit = 10;
  const authoritative = true;
  const label = "";
  const minSize = 0;
  const maxSize = (request.maxPlayers || 9) - 1;
  const matches = nk.matchList(limit, authoritative, label, minSize, maxSize, "");
  for (const match of matches) {
    try {
      const matchLabel = JSON.parse(match.label || "{}");
      if (request.blinds && matchLabel.blinds !== request.blinds) {
        continue;
      }
      if (matchLabel.players < matchLabel.maxPlayers) {
        logger.info("Found existing match", { matchId: match.matchId });
        const response2 = {
          matchId: match.matchId,
          label: match.label || "",
          created: false
        };
        return JSON.stringify(response2);
      }
    } catch (e) {
      logger.warn("Failed to parse match label", { matchId: match.matchId, error: e });
    }
  }
  logger.info("No suitable match found, creating new one");
  const params = {
    label: "Texas Hold'em"
  };
  if (request.minPlayers) {
    params.minPlayers = request.minPlayers.toString();
  }
  if (request.maxPlayers) {
    params.maxPlayers = request.maxPlayers.toString();
  }
  if (request.blinds) {
    const [small, big] = request.blinds.split("/");
    params.smallBlind = small;
    params.bigBlind = big;
  }
  const matchId = nk.matchCreate(POKER_MATCH_MODULE, params);
  logger.info("Created new match", { matchId });
  const response = {
    matchId,
    label: params.label,
    created: true
  };
  return JSON.stringify(response);
};
var listRoomsRpc = function(ctx, logger, nk, payload) {
  logger.info("List rooms RPC called", { userId: ctx.userId });
  const limit = 50;
  const authoritative = true;
  const label = "";
  const minSize = 0;
  const maxSize = 9;
  const matches = nk.matchList(limit, authoritative, label, minSize, maxSize, "");
  const rooms = [];
  for (const match of matches) {
    try {
      const matchLabel = JSON.parse(match.label || "{}");
      rooms.push({
        matchId: match.matchId,
        label: matchLabel.name || "Texas Hold'em",
        players: matchLabel.players || match.size,
        maxPlayers: matchLabel.maxPlayers || 9,
        spectators: matchLabel.spectators || 0,
        blinds: matchLabel.blinds || "10/20",
        phase: matchLabel.phase || "waiting",
        createdAt: matchLabel.createdAt
      });
    } catch (e) {
      logger.warn("Failed to parse match label", { matchId: match.matchId, error: e });
      rooms.push({
        matchId: match.matchId,
        label: "Texas Hold'em",
        players: match.size,
        maxPlayers: 9,
        spectators: 0,
        blinds: "10/20",
        phase: "unknown"
      });
    }
  }
  rooms.sort((a, b) => {
    if (b.players !== a.players) {
      return b.players - a.players;
    }
    return (b.createdAt || 0) - (a.createdAt || 0);
  });
  const response = {
    rooms,
    total: rooms.length
  };
  logger.info("Listed rooms", { count: rooms.length });
  return JSON.stringify(response);
};
var createPrivateMatchRpc = function(ctx, logger, nk, payload) {
  logger.info("Create private match RPC called", { userId: ctx.userId, payload });
  let request = {};
  if (payload && payload.length > 0) {
    try {
      request = JSON.parse(payload);
    } catch (e) {
      logger.error("Failed to parse payload", { error: e });
    }
  }
  const params = {
    label: request.label || "Private Game"
  };
  if (request.minPlayers) {
    params.minPlayers = request.minPlayers.toString();
  }
  if (request.maxPlayers) {
    params.maxPlayers = request.maxPlayers.toString();
  }
  if (request.smallBlind) {
    params.smallBlind = request.smallBlind.toString();
  }
  if (request.bigBlind) {
    params.bigBlind = request.bigBlind.toString();
  }
  if (request.startingChips) {
    params.startingChips = request.startingChips.toString();
  }
  const matchId = nk.matchCreate(POKER_MATCH_MODULE, params);
  logger.info("Created private match", { matchId, params });
  return JSON.stringify({
    matchId,
    label: params.label
  });
};

// src/main.ts
var POKER_MATCH_MODULE2 = "poker";
function InitModule(ctx, logger, nk, initializer) {
  logger.info("Poker server initializing...");
  initializer.registerMatch(POKER_MATCH_MODULE2, pokerMatchHandler);
  logger.info("Registered poker match handler");
  initializer.registerRpc("find_match", findMatchRpc);
  initializer.registerRpc("create_private_match", createPrivateMatchRpc);
  initializer.registerRpc("list_rooms", listRoomsRpc);
  logger.info("Registered RPC functions");
  logger.info("Poker server initialized successfully!");
}
globalThis.InitModule = InitModule;
