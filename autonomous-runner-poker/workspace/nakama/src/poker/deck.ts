/**
 * Deck - Standard 52-card deck for Texas Hold'em
 */

import { Card, Suit, Rank } from './types';

// All suits in a standard deck
const SUITS: Suit[] = [Suit.Hearts, Suit.Diamonds, Suit.Clubs, Suit.Spades];

// All ranks (2-14, where 11=J, 12=Q, 13=K, 14=A)
const RANKS: Rank[] = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];

/**
 * Creates a standard 52-card deck
 */
export function createDeck(): Card[] {
  const deck: Card[] = [];
  for (const suit of SUITS) {
    for (const rank of RANKS) {
      deck.push({ suit, rank });
    }
  }
  return deck;
}

/**
 * Fisher-Yates shuffle algorithm
 * Shuffles the deck in place and returns it
 */
export function shuffleDeck(deck: Card[]): Card[] {
  for (let i = deck.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [deck[i], deck[j]] = [deck[j], deck[i]];
  }
  return deck;
}

/**
 * Deal cards from the top of the deck
 * Returns the dealt cards and removes them from the deck
 */
export function dealCards(deck: Card[], count: number): Card[] {
  if (count > deck.length) {
    throw new Error(`Cannot deal ${count} cards, only ${deck.length} remaining`);
  }
  return deck.splice(0, count);
}

/**
 * Deal a single card from the top of the deck
 */
export function dealCard(deck: Card[]): Card {
  if (deck.length === 0) {
    throw new Error('Cannot deal from empty deck');
  }
  return deck.shift()!;
}

/**
 * Get remaining cards in deck
 */
export function getRemainingCount(deck: Card[]): number {
  return deck.length;
}

/**
 * Create a new shuffled deck ready for play
 */
export function createShuffledDeck(): Card[] {
  return shuffleDeck(createDeck());
}

/**
 * Convert card to readable string (for debugging)
 */
export function cardToString(card: Card): string {
  const rankNames: { [key: number]: string } = {
    2: '2', 3: '3', 4: '4', 5: '5', 6: '6', 7: '7', 8: '8', 9: '9',
    10: '10', 11: 'J', 12: 'Q', 13: 'K', 14: 'A'
  };
  const suitSymbols: { [key in Suit]: string } = {
    [Suit.Hearts]: '♥',
    [Suit.Diamonds]: '♦',
    [Suit.Clubs]: '♣',
    [Suit.Spades]: '♠'
  };
  return `${rankNames[card.rank]}${suitSymbols[card.suit]}`;
}

/**
 * Convert multiple cards to readable string
 */
export function cardsToString(cards: Card[]): string {
  return cards.map(cardToString).join(' ');
}

/**
 * Deck class for object-oriented usage (alternative API)
 */
export class Deck {
  private cards: Card[];

  constructor() {
    this.cards = createShuffledDeck();
  }

  /**
   * Shuffle the deck
   */
  shuffle(): this {
    shuffleDeck(this.cards);
    return this;
  }

  /**
   * Reset deck to a new shuffled 52-card deck
   */
  reset(): this {
    this.cards = createShuffledDeck();
    return this;
  }

  /**
   * Deal multiple cards
   */
  deal(count: number): Card[] {
    return dealCards(this.cards, count);
  }

  /**
   * Deal a single card
   */
  dealOne(): Card {
    return dealCard(this.cards);
  }

  /**
   * Get number of remaining cards
   */
  get remaining(): number {
    return this.cards.length;
  }

  /**
   * Get all remaining cards (for debugging)
   */
  getCards(): Card[] {
    return [...this.cards];
  }

  /**
   * String representation
   */
  toString(): string {
    return `Deck(${this.remaining} cards)`;
  }
}
