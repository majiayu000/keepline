/**
 * Unit tests for Deck module - Standard 52-card deck for Texas Hold'em
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  createDeck,
  shuffleDeck,
  dealCards,
  dealCard,
  getRemainingCount,
  createShuffledDeck,
  cardToString,
  cardsToString,
  Deck
} from './deck';
import { Card, Suit, Rank } from './types';

// Shorthand for suits
const H = Suit.Hearts;
const D = Suit.Diamonds;
const C = Suit.Clubs;
const S = Suit.Spades;

// Helper function to create a card
function card(rank: Rank, suit: Suit): Card {
  return { rank, suit };
}

describe('Deck Module', () => {
  describe('createDeck', () => {
    it('should create a deck with exactly 52 cards', () => {
      const deck = createDeck();
      expect(deck.length).toBe(52);
    });

    it('should contain all 4 suits', () => {
      const deck = createDeck();
      const suits = new Set(deck.map(c => c.suit));
      expect(suits.size).toBe(4);
      expect(suits.has(Suit.Hearts)).toBe(true);
      expect(suits.has(Suit.Diamonds)).toBe(true);
      expect(suits.has(Suit.Clubs)).toBe(true);
      expect(suits.has(Suit.Spades)).toBe(true);
    });

    it('should contain all 13 ranks for each suit', () => {
      const deck = createDeck();
      const expectedRanks: Rank[] = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];

      for (const suit of [H, D, C, S]) {
        const cardsOfSuit = deck.filter(c => c.suit === suit);
        expect(cardsOfSuit.length).toBe(13);

        const ranks = cardsOfSuit.map(c => c.rank).sort((a, b) => a - b);
        expect(ranks).toEqual(expectedRanks);
      }
    });

    it('should have no duplicate cards', () => {
      const deck = createDeck();
      const cardStrings = deck.map(c => `${c.rank}-${c.suit}`);
      const uniqueCards = new Set(cardStrings);
      expect(uniqueCards.size).toBe(52);
    });

    it('should create cards with rank values from 2 to 14', () => {
      const deck = createDeck();
      const minRank = Math.min(...deck.map(c => c.rank));
      const maxRank = Math.max(...deck.map(c => c.rank));
      expect(minRank).toBe(2);
      expect(maxRank).toBe(14); // Ace
    });

    it('should create a new array each time', () => {
      const deck1 = createDeck();
      const deck2 = createDeck();
      expect(deck1).not.toBe(deck2);
      expect(deck1[0]).not.toBe(deck2[0]); // Different card objects
    });
  });

  describe('shuffleDeck', () => {
    it('should maintain the same number of cards after shuffle', () => {
      const deck = createDeck();
      shuffleDeck(deck);
      expect(deck.length).toBe(52);
    });

    it('should contain all original cards after shuffle', () => {
      const deck = createDeck();
      const originalCards = deck.map(c => `${c.rank}-${c.suit}`).sort();

      shuffleDeck(deck);
      const shuffledCards = deck.map(c => `${c.rank}-${c.suit}`).sort();

      expect(shuffledCards).toEqual(originalCards);
    });

    it('should shuffle in place and return the same array', () => {
      const deck = createDeck();
      const returnedDeck = shuffleDeck(deck);
      expect(returnedDeck).toBe(deck);
    });

    it('should produce different order than original (with high probability)', () => {
      const deck1 = createDeck();
      const originalOrder = deck1.map(c => `${c.rank}-${c.suit}`).join(',');

      shuffleDeck(deck1);
      const shuffledOrder = deck1.map(c => `${c.rank}-${c.suit}`).join(',');

      // Very unlikely to have the same order after shuffle
      expect(shuffledOrder).not.toBe(originalOrder);
    });

    it('should handle empty deck', () => {
      const deck: Card[] = [];
      const result = shuffleDeck(deck);
      expect(result.length).toBe(0);
    });

    it('should handle single card deck', () => {
      const deck: Card[] = [card(14, H)];
      const result = shuffleDeck(deck);
      expect(result.length).toBe(1);
      expect(result[0]).toEqual(card(14, H));
    });

    it('should handle two card deck', () => {
      const deck: Card[] = [card(14, H), card(2, S)];
      shuffleDeck(deck);
      expect(deck.length).toBe(2);
      // Both cards should still be present
      const cardStrings = deck.map(c => `${c.rank}-${c.suit}`).sort();
      expect(cardStrings).toEqual(['14-hearts', '2-spades']);
    });

    it('should produce different results on multiple shuffles (randomness test)', () => {
      // Shuffle many times and verify we get different results
      const results = new Set<string>();
      for (let i = 0; i < 10; i++) {
        const deck = createDeck();
        shuffleDeck(deck);
        const order = deck.slice(0, 5).map(c => `${c.rank}-${c.suit}`).join(',');
        results.add(order);
      }
      // Should have multiple different results (very unlikely to have all same)
      expect(results.size).toBeGreaterThan(1);
    });
  });

  describe('dealCards', () => {
    it('should deal the specified number of cards', () => {
      const deck = createDeck();
      const dealt = dealCards(deck, 5);
      expect(dealt.length).toBe(5);
    });

    it('should remove dealt cards from the deck', () => {
      const deck = createDeck();
      dealCards(deck, 5);
      expect(deck.length).toBe(47);
    });

    it('should deal from the top of the deck', () => {
      const deck = createDeck();
      const topCards = deck.slice(0, 3).map(c => ({ ...c }));
      const dealt = dealCards(deck, 3);

      expect(dealt[0]).toEqual(topCards[0]);
      expect(dealt[1]).toEqual(topCards[1]);
      expect(dealt[2]).toEqual(topCards[2]);
    });

    it('should deal all remaining cards when count equals deck size', () => {
      const deck = createDeck();
      const dealt = dealCards(deck, 52);
      expect(dealt.length).toBe(52);
      expect(deck.length).toBe(0);
    });

    it('should throw error when dealing more cards than available', () => {
      const deck = createDeck();
      dealCards(deck, 50);

      expect(() => dealCards(deck, 5)).toThrow('Cannot deal 5 cards, only 2 remaining');
    });

    it('should throw error when deck is empty', () => {
      const deck: Card[] = [];
      expect(() => dealCards(deck, 1)).toThrow('Cannot deal 1 cards, only 0 remaining');
    });

    it('should allow dealing 0 cards', () => {
      const deck = createDeck();
      const dealt = dealCards(deck, 0);
      expect(dealt.length).toBe(0);
      expect(deck.length).toBe(52);
    });

    it('should return new array (not reference to deck portion)', () => {
      const deck = createDeck();
      const dealt = dealCards(deck, 5);
      dealt[0] = card(2, H); // Modify dealt cards
      // Original deck should not be affected beyond removal
      expect(deck.length).toBe(47);
    });
  });

  describe('dealCard', () => {
    it('should deal a single card', () => {
      const deck = createDeck();
      const card = dealCard(deck);
      expect(card).toBeDefined();
      expect(card.rank).toBeDefined();
      expect(card.suit).toBeDefined();
    });

    it('should remove the dealt card from deck', () => {
      const deck = createDeck();
      dealCard(deck);
      expect(deck.length).toBe(51);
    });

    it('should deal from the top of the deck', () => {
      const deck = createDeck();
      const topCard = { ...deck[0] };
      const dealt = dealCard(deck);
      expect(dealt).toEqual(topCard);
    });

    it('should deal cards in order from top', () => {
      const deck = createDeck();
      const firstThree = [
        { ...deck[0] },
        { ...deck[1] },
        { ...deck[2] }
      ];

      expect(dealCard(deck)).toEqual(firstThree[0]);
      expect(dealCard(deck)).toEqual(firstThree[1]);
      expect(dealCard(deck)).toEqual(firstThree[2]);
      expect(deck.length).toBe(49);
    });

    it('should throw error when deck is empty', () => {
      const deck: Card[] = [];
      expect(() => dealCard(deck)).toThrow('Cannot deal from empty deck');
    });

    it('should deal all cards one by one', () => {
      const deck = createDeck();
      const dealtCards: Card[] = [];

      while (deck.length > 0) {
        dealtCards.push(dealCard(deck));
      }

      expect(dealtCards.length).toBe(52);
      expect(deck.length).toBe(0);
    });
  });

  describe('getRemainingCount', () => {
    it('should return 52 for a new deck', () => {
      const deck = createDeck();
      expect(getRemainingCount(deck)).toBe(52);
    });

    it('should return correct count after dealing', () => {
      const deck = createDeck();
      dealCards(deck, 10);
      expect(getRemainingCount(deck)).toBe(42);
    });

    it('should return 0 for empty deck', () => {
      const deck: Card[] = [];
      expect(getRemainingCount(deck)).toBe(0);
    });

    it('should track count accurately through multiple deals', () => {
      const deck = createDeck();

      dealCards(deck, 5);
      expect(getRemainingCount(deck)).toBe(47);

      dealCard(deck);
      expect(getRemainingCount(deck)).toBe(46);

      dealCards(deck, 10);
      expect(getRemainingCount(deck)).toBe(36);
    });
  });

  describe('createShuffledDeck', () => {
    it('should create a deck with 52 cards', () => {
      const deck = createShuffledDeck();
      expect(deck.length).toBe(52);
    });

    it('should contain all unique cards', () => {
      const deck = createShuffledDeck();
      const cardStrings = deck.map(c => `${c.rank}-${c.suit}`);
      const uniqueCards = new Set(cardStrings);
      expect(uniqueCards.size).toBe(52);
    });

    it('should be shuffled (different from standard order)', () => {
      const standardDeck = createDeck();
      const shuffledDeck = createShuffledDeck();

      const standardOrder = standardDeck.map(c => `${c.rank}-${c.suit}`).join(',');
      const shuffledOrder = shuffledDeck.map(c => `${c.rank}-${c.suit}`).join(',');

      expect(shuffledOrder).not.toBe(standardOrder);
    });

    it('should produce different orders on each call (usually)', () => {
      const deck1 = createShuffledDeck();
      const deck2 = createShuffledDeck();

      const order1 = deck1.map(c => `${c.rank}-${c.suit}`).join(',');
      const order2 = deck2.map(c => `${c.rank}-${c.suit}`).join(',');

      expect(order1).not.toBe(order2);
    });

    it('should return a new array each time', () => {
      const deck1 = createShuffledDeck();
      const deck2 = createShuffledDeck();
      expect(deck1).not.toBe(deck2);
    });
  });

  describe('cardToString', () => {
    it('should convert number cards correctly', () => {
      expect(cardToString(card(2, H))).toBe('2\u2665');
      expect(cardToString(card(5, D))).toBe('5\u2666');
      expect(cardToString(card(9, C))).toBe('9\u2663');
      expect(cardToString(card(10, S))).toBe('10\u2660');
    });

    it('should convert face cards correctly', () => {
      expect(cardToString(card(11, H))).toBe('J\u2665'); // Jack
      expect(cardToString(card(12, D))).toBe('Q\u2666'); // Queen
      expect(cardToString(card(13, C))).toBe('K\u2663'); // King
      expect(cardToString(card(14, S))).toBe('A\u2660'); // Ace
    });

    it('should use correct suit symbols', () => {
      expect(cardToString(card(2, Suit.Hearts))).toContain('\u2665'); // ♥
      expect(cardToString(card(2, Suit.Diamonds))).toContain('\u2666'); // ♦
      expect(cardToString(card(2, Suit.Clubs))).toContain('\u2663'); // ♣
      expect(cardToString(card(2, Suit.Spades))).toContain('\u2660'); // ♠
    });

    it('should convert all Aces correctly', () => {
      expect(cardToString(card(14, H))).toBe('A\u2665');
      expect(cardToString(card(14, D))).toBe('A\u2666');
      expect(cardToString(card(14, C))).toBe('A\u2663');
      expect(cardToString(card(14, S))).toBe('A\u2660');
    });

    it('should convert a sequence of cards', () => {
      // A full straight
      expect(cardToString(card(10, H))).toBe('10\u2665');
      expect(cardToString(card(11, H))).toBe('J\u2665');
      expect(cardToString(card(12, H))).toBe('Q\u2665');
      expect(cardToString(card(13, H))).toBe('K\u2665');
      expect(cardToString(card(14, H))).toBe('A\u2665');
    });
  });

  describe('cardsToString', () => {
    it('should convert empty array to empty string', () => {
      expect(cardsToString([])).toBe('');
    });

    it('should convert single card', () => {
      expect(cardsToString([card(14, H)])).toBe('A\u2665');
    });

    it('should convert multiple cards with space separator', () => {
      const cards = [card(14, H), card(13, H), card(12, H)];
      expect(cardsToString(cards)).toBe('A\u2665 K\u2665 Q\u2665');
    });

    it('should convert a poker hand correctly', () => {
      // Royal flush
      const royalFlush = [
        card(14, S), card(13, S), card(12, S), card(11, S), card(10, S)
      ];
      expect(cardsToString(royalFlush)).toBe('A\u2660 K\u2660 Q\u2660 J\u2660 10\u2660');
    });

    it('should convert mixed suits correctly', () => {
      const mixedCards = [
        card(14, H), card(14, D), card(14, C), card(14, S)
      ];
      expect(cardsToString(mixedCards)).toBe('A\u2665 A\u2666 A\u2663 A\u2660');
    });

    it('should handle hole cards (2 cards)', () => {
      const holeCards = [card(14, H), card(14, S)];
      expect(cardsToString(holeCards)).toBe('A\u2665 A\u2660');
    });

    it('should handle community cards (5 cards)', () => {
      const communityCards = [
        card(2, H), card(5, D), card(9, C), card(11, S), card(13, H)
      ];
      expect(cardsToString(communityCards)).toBe('2\u2665 5\u2666 9\u2663 J\u2660 K\u2665');
    });
  });

  describe('Deck Class', () => {
    describe('constructor', () => {
      it('should create a deck with 52 cards', () => {
        const deck = new Deck();
        expect(deck.remaining).toBe(52);
      });

      it('should be shuffled on creation', () => {
        const standardDeck = createDeck();
        const deck = new Deck();

        const standardOrder = standardDeck.map(c => `${c.rank}-${c.suit}`).join(',');
        const deckOrder = deck.getCards().map(c => `${c.rank}-${c.suit}`).join(',');

        expect(deckOrder).not.toBe(standardOrder);
      });
    });

    describe('shuffle', () => {
      it('should shuffle the deck', () => {
        const deck = new Deck();
        const beforeShuffle = deck.getCards().map(c => `${c.rank}-${c.suit}`).join(',');

        deck.shuffle();
        const afterShuffle = deck.getCards().map(c => `${c.rank}-${c.suit}`).join(',');

        // Unlikely to be the same
        expect(afterShuffle).not.toBe(beforeShuffle);
      });

      it('should maintain 52 cards after shuffle', () => {
        const deck = new Deck();
        deck.deal(10);
        deck.shuffle();
        expect(deck.remaining).toBe(42);
      });

      it('should return this for chaining', () => {
        const deck = new Deck();
        expect(deck.shuffle()).toBe(deck);
      });
    });

    describe('reset', () => {
      it('should reset to 52 cards', () => {
        const deck = new Deck();
        deck.deal(30);
        expect(deck.remaining).toBe(22);

        deck.reset();
        expect(deck.remaining).toBe(52);
      });

      it('should return this for chaining', () => {
        const deck = new Deck();
        expect(deck.reset()).toBe(deck);
      });

      it('should allow dealing after reset', () => {
        const deck = new Deck();
        deck.deal(52);
        expect(deck.remaining).toBe(0);

        deck.reset();
        const cards = deck.deal(5);
        expect(cards.length).toBe(5);
        expect(deck.remaining).toBe(47);
      });

      it('should create new shuffled deck', () => {
        const deck = new Deck();
        const orderBefore = deck.getCards().map(c => `${c.rank}-${c.suit}`).join(',');

        deck.reset();
        const orderAfter = deck.getCards().map(c => `${c.rank}-${c.suit}`).join(',');

        // Orders should be different (both shuffled independently)
        expect(orderAfter).not.toBe(orderBefore);
      });
    });

    describe('deal', () => {
      it('should deal specified number of cards', () => {
        const deck = new Deck();
        const cards = deck.deal(5);
        expect(cards.length).toBe(5);
      });

      it('should reduce remaining count', () => {
        const deck = new Deck();
        deck.deal(10);
        expect(deck.remaining).toBe(42);
      });

      it('should throw when dealing too many cards', () => {
        const deck = new Deck();
        deck.deal(50);

        expect(() => deck.deal(5)).toThrow();
      });
    });

    describe('dealOne', () => {
      it('should deal a single card', () => {
        const deck = new Deck();
        const card = deck.dealOne();

        expect(card).toBeDefined();
        expect(card.rank).toBeDefined();
        expect(card.suit).toBeDefined();
      });

      it('should reduce remaining by 1', () => {
        const deck = new Deck();
        deck.dealOne();
        expect(deck.remaining).toBe(51);
      });

      it('should throw when deck is empty', () => {
        const deck = new Deck();
        for (let i = 0; i < 52; i++) {
          deck.dealOne();
        }

        expect(() => deck.dealOne()).toThrow('Cannot deal from empty deck');
      });
    });

    describe('remaining', () => {
      it('should return correct count', () => {
        const deck = new Deck();
        expect(deck.remaining).toBe(52);

        deck.deal(5);
        expect(deck.remaining).toBe(47);

        deck.dealOne();
        expect(deck.remaining).toBe(46);
      });
    });

    describe('getCards', () => {
      it('should return all remaining cards', () => {
        const deck = new Deck();
        const cards = deck.getCards();
        expect(cards.length).toBe(52);
      });

      it('should return a copy (not the internal array)', () => {
        const deck = new Deck();
        const cards = deck.getCards();
        cards.pop(); // Modify the returned array

        expect(deck.remaining).toBe(52); // Internal deck unchanged
      });

      it('should reflect deals', () => {
        const deck = new Deck();
        deck.deal(10);
        const cards = deck.getCards();
        expect(cards.length).toBe(42);
      });
    });

    describe('toString', () => {
      it('should return deck representation', () => {
        const deck = new Deck();
        expect(deck.toString()).toBe('Deck(52 cards)');
      });

      it('should show correct remaining count', () => {
        const deck = new Deck();
        deck.deal(20);
        expect(deck.toString()).toBe('Deck(32 cards)');
      });

      it('should show 0 cards when empty', () => {
        const deck = new Deck();
        deck.deal(52);
        expect(deck.toString()).toBe('Deck(0 cards)');
      });
    });

    describe('chaining', () => {
      it('should allow method chaining', () => {
        const deck = new Deck();
        deck.shuffle().reset().shuffle();
        expect(deck.remaining).toBe(52);
      });
    });
  });

  describe('Integration: Full Game Simulation', () => {
    it('should support a Texas Holdem hand deal', () => {
      const deck = new Deck();

      // Deal hole cards to 4 players
      const player1 = deck.deal(2);
      const player2 = deck.deal(2);
      const player3 = deck.deal(2);
      const player4 = deck.deal(2);

      expect(player1.length).toBe(2);
      expect(player2.length).toBe(2);
      expect(player3.length).toBe(2);
      expect(player4.length).toBe(2);
      expect(deck.remaining).toBe(44);

      // Burn and deal flop
      deck.dealOne(); // burn
      const flop = deck.deal(3);
      expect(flop.length).toBe(3);
      expect(deck.remaining).toBe(40);

      // Burn and deal turn
      deck.dealOne(); // burn
      const turn = deck.dealOne();
      expect(turn).toBeDefined();
      expect(deck.remaining).toBe(38);

      // Burn and deal river
      deck.dealOne(); // burn
      const river = deck.dealOne();
      expect(river).toBeDefined();
      expect(deck.remaining).toBe(36);

      // Verify all player-visible cards are unique
      const allCards = [
        ...player1, ...player2, ...player3, ...player4,
        ...flop, turn, river
      ];
      const cardStrings = allCards.map(c => `${c.rank}-${c.suit}`);
      const uniqueCards = new Set(cardStrings);
      expect(uniqueCards.size).toBe(13); // 4*2 + 3 + 1 + 1 = 8 + 5 = 13 cards (not counting burns)
    });

    it('should support maximum 9 players Texas Holdem', () => {
      const deck = new Deck();

      // Deal to 9 players
      const players: Card[][] = [];
      for (let i = 0; i < 9; i++) {
        players.push(deck.deal(2));
      }

      expect(deck.remaining).toBe(34); // 52 - 18 = 34

      // Burn cards (3) + community cards (5) = 8
      deck.dealOne(); // burn
      deck.deal(3);   // flop
      deck.dealOne(); // burn
      deck.dealOne(); // turn
      deck.dealOne(); // burn
      deck.dealOne(); // river

      expect(deck.remaining).toBe(26); // 34 - 8 = 26

      // All dealt cards should be unique
      const allCards: Card[] = [];
      players.forEach(p => allCards.push(...p));
      const cardStrings = allCards.map(c => `${c.rank}-${c.suit}`);
      expect(new Set(cardStrings).size).toBe(18);
    });

    it('should be ready for new hand after reset', () => {
      const deck = new Deck();

      // Play one hand
      deck.deal(18); // 9 players
      deck.deal(8);  // burns + community
      expect(deck.remaining).toBe(26);

      // Reset for new hand
      deck.reset();
      expect(deck.remaining).toBe(52);

      // Should work normally
      const cards = deck.deal(10);
      expect(cards.length).toBe(10);
    });
  });

  describe('Edge Cases', () => {
    it('should handle dealing exact remaining cards', () => {
      const deck = createDeck();
      dealCards(deck, 50);
      const lastTwo = dealCards(deck, 2);

      expect(lastTwo.length).toBe(2);
      expect(deck.length).toBe(0);
    });

    it('should not modify original deck when getting cards from Deck class', () => {
      const deck = new Deck();
      const cards = deck.getCards();

      // Verify they are copies
      cards[0].rank = 2 as Rank;
      cards[0].suit = Suit.Hearts;

      // Internal state should not be affected
      const freshCards = deck.getCards();
      expect(freshCards.length).toBe(52);
    });

    it('should handle rapid shuffles', () => {
      const deck = new Deck();
      for (let i = 0; i < 100; i++) {
        deck.shuffle();
      }

      expect(deck.remaining).toBe(52);

      // Should still have all unique cards
      const cardStrings = deck.getCards().map(c => `${c.rank}-${c.suit}`);
      expect(new Set(cardStrings).size).toBe(52);
    });
  });
});
