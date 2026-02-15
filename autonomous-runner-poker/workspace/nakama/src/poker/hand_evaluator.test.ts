/**
 * Unit tests for Texas Hold'em Hand Evaluator
 */

import { describe, it, expect } from 'vitest';
import {
  evaluateHand,
  compareEvaluatedHands,
  compareHands,
  determineWinners,
  getHandStrength,
  getHandRankName
} from './hand_evaluator';
import { Card, Suit, Rank, HandRank } from './types';

// Helper function to create cards
function card(rank: Rank, suit: Suit): Card {
  return { rank, suit };
}

// Shorthand for suits
const H = Suit.Hearts;
const D = Suit.Diamonds;
const C = Suit.Clubs;
const S = Suit.Spades;

describe('Hand Evaluator', () => {
  describe('evaluateHand - Hand Type Recognition', () => {
    it('should identify Royal Flush', () => {
      const cards: Card[] = [
        card(14, H), card(13, H), card(12, H), card(11, H), card(10, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.RoyalFlush);
      expect(result.description).toBe('Royal Flush');
    });

    it('should identify Straight Flush', () => {
      const cards: Card[] = [
        card(9, S), card(8, S), card(7, S), card(6, S), card(5, S)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.StraightFlush);
      expect(result.description).toContain('Straight Flush');
    });

    it('should identify Wheel Straight Flush (A-2-3-4-5)', () => {
      const cards: Card[] = [
        card(14, D), card(2, D), card(3, D), card(4, D), card(5, D)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.StraightFlush);
      expect(result.kickers[0]).toBe(5); // 5-high
    });

    it('should identify Four of a Kind', () => {
      const cards: Card[] = [
        card(8, H), card(8, D), card(8, C), card(8, S), card(3, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.FourOfAKind);
      expect(result.kickers[0]).toBe(8);
    });

    it('should identify Full House', () => {
      const cards: Card[] = [
        card(10, H), card(10, D), card(10, C), card(7, H), card(7, D)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.FullHouse);
      expect(result.kickers).toEqual([10, 7]);
    });

    it('should identify Flush', () => {
      const cards: Card[] = [
        card(14, C), card(11, C), card(8, C), card(5, C), card(2, C)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.Flush);
      expect(result.kickers[0]).toBe(14);
    });

    it('should identify Straight', () => {
      const cards: Card[] = [
        card(9, H), card(8, D), card(7, C), card(6, S), card(5, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.Straight);
      expect(result.kickers[0]).toBe(9);
    });

    it('should identify Wheel Straight (A-2-3-4-5)', () => {
      const cards: Card[] = [
        card(14, H), card(2, D), card(3, C), card(4, S), card(5, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.Straight);
      expect(result.kickers[0]).toBe(5); // 5-high straight
    });

    it('should identify Broadway Straight (10-J-Q-K-A)', () => {
      const cards: Card[] = [
        card(14, H), card(13, D), card(12, C), card(11, S), card(10, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.Straight);
      expect(result.kickers[0]).toBe(14);
    });

    it('should identify Three of a Kind', () => {
      const cards: Card[] = [
        card(6, H), card(6, D), card(6, C), card(11, S), card(3, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.ThreeOfAKind);
      expect(result.kickers[0]).toBe(6);
    });

    it('should identify Two Pair', () => {
      const cards: Card[] = [
        card(12, H), card(12, D), card(8, C), card(8, S), card(5, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.TwoPair);
      expect(result.kickers).toEqual([12, 8, 5]);
    });

    it('should identify One Pair', () => {
      const cards: Card[] = [
        card(9, H), card(9, D), card(13, C), card(7, S), card(2, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.OnePair);
      expect(result.kickers[0]).toBe(9);
    });

    it('should identify High Card', () => {
      const cards: Card[] = [
        card(14, H), card(10, D), card(8, C), card(5, S), card(2, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.HighCard);
      expect(result.kickers[0]).toBe(14);
    });
  });

  describe('evaluateHand - 7 Card Selection', () => {
    it('should find best 5 cards from 7 cards', () => {
      // Hole cards: A♥ K♥, Community: Q♥ J♥ 10♥ 2♣ 3♦
      const cards: Card[] = [
        card(14, H), card(13, H), // hole
        card(12, H), card(11, H), card(10, H), card(2, C), card(3, D) // community
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.RoyalFlush);
    });

    it('should select full house over flush when available', () => {
      // A full house beats a flush
      const cards: Card[] = [
        card(10, H), card(10, D), card(10, C), // trips
        card(7, H), card(7, D), // pair
        card(5, H), card(2, H) // extra hearts for potential flush
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.FullHouse);
    });

    it('should select the higher full house', () => {
      // Two potential full houses: AAA+KK or KKK+AA, should pick AAA+KK
      const cards: Card[] = [
        card(14, H), card(14, D), card(14, C), // AAA
        card(13, H), card(13, D), card(13, C), // KKK
        card(2, S) // random
      ];
      const result = evaluateHand(cards);
      // Best hand is Aces full of Kings (AAA + KK)
      expect(result.rank).toBe(HandRank.FullHouse);
      expect(result.kickers[0]).toBe(14); // Aces full
      expect(result.kickers[1]).toBe(13); // of Kings
    });
  });

  describe('compareEvaluatedHands', () => {
    it('should rank hands correctly by type', () => {
      const royalFlush = evaluateHand([
        card(14, H), card(13, H), card(12, H), card(11, H), card(10, H)
      ]);
      const straightFlush = evaluateHand([
        card(9, S), card(8, S), card(7, S), card(6, S), card(5, S)
      ]);
      const fourOfAKind = evaluateHand([
        card(8, H), card(8, D), card(8, C), card(8, S), card(14, H)
      ]);
      const fullHouse = evaluateHand([
        card(10, H), card(10, D), card(10, C), card(7, H), card(7, D)
      ]);

      expect(compareEvaluatedHands(royalFlush, straightFlush)).toBe(1);
      expect(compareEvaluatedHands(straightFlush, fourOfAKind)).toBe(1);
      expect(compareEvaluatedHands(fourOfAKind, fullHouse)).toBe(1);
    });

    it('should compare same hand type by kickers', () => {
      const pairOfKings = evaluateHand([
        card(13, H), card(13, D), card(10, C), card(7, S), card(2, H)
      ]);
      const pairOfQueens = evaluateHand([
        card(12, H), card(12, D), card(10, C), card(7, S), card(2, H)
      ]);

      expect(compareEvaluatedHands(pairOfKings, pairOfQueens)).toBe(1);
      expect(compareEvaluatedHands(pairOfQueens, pairOfKings)).toBe(-1);
    });

    it('should detect ties', () => {
      const hand1 = evaluateHand([
        card(14, H), card(13, D), card(10, C), card(7, S), card(2, H)
      ]);
      const hand2 = evaluateHand([
        card(14, D), card(13, C), card(10, S), card(7, H), card(2, D)
      ]);

      expect(compareEvaluatedHands(hand1, hand2)).toBe(0);
    });

    it('should compare two pair correctly', () => {
      // Kings and Tens vs Queens and Jacks
      const kingsAndTens = evaluateHand([
        card(13, H), card(13, D), card(10, C), card(10, S), card(5, H)
      ]);
      const queensAndJacks = evaluateHand([
        card(12, H), card(12, D), card(11, C), card(11, S), card(5, D)
      ]);

      expect(compareEvaluatedHands(kingsAndTens, queensAndJacks)).toBe(1);
    });

    it('should compare two pair with same high pair by low pair', () => {
      const kingsAndTens = evaluateHand([
        card(13, H), card(13, D), card(10, C), card(10, S), card(5, H)
      ]);
      const kingsAndNines = evaluateHand([
        card(13, C), card(13, S), card(9, C), card(9, S), card(5, D)
      ]);

      expect(compareEvaluatedHands(kingsAndTens, kingsAndNines)).toBe(1);
    });

    it('should compare straights by high card', () => {
      const nineHigh = evaluateHand([
        card(9, H), card(8, D), card(7, C), card(6, S), card(5, H)
      ]);
      const eightHigh = evaluateHand([
        card(8, H), card(7, D), card(6, C), card(5, S), card(4, H)
      ]);

      expect(compareEvaluatedHands(nineHigh, eightHigh)).toBe(1);
    });

    it('should rank Broadway straight over wheel', () => {
      const broadway = evaluateHand([
        card(14, H), card(13, D), card(12, C), card(11, S), card(10, H)
      ]);
      const wheel = evaluateHand([
        card(14, D), card(2, C), card(3, S), card(4, H), card(5, D)
      ]);

      expect(compareEvaluatedHands(broadway, wheel)).toBe(1);
    });
  });

  describe('compareHands - Full Hand Comparison', () => {
    it('should compare raw hands (7 cards)', () => {
      // Player 1: Flush
      const hand1: Card[] = [
        card(14, H), card(10, H),
        card(8, H), card(5, H), card(2, H), card(7, D), card(3, S)
      ];
      // Player 2: Straight
      const hand2: Card[] = [
        card(9, H), card(8, D),
        card(7, C), card(6, S), card(5, H), card(2, D), card(3, C)
      ];

      expect(compareHands(hand1, hand2)).toBe(1);
    });
  });

  describe('determineWinners', () => {
    it('should determine single winner', () => {
      const hands: Card[][] = [
        [card(14, H), card(14, D), card(10, C), card(7, S), card(2, H)], // Pair of Aces
        [card(13, H), card(13, D), card(10, C), card(7, S), card(2, H)], // Pair of Kings
        [card(12, H), card(12, D), card(10, C), card(7, S), card(2, H)]  // Pair of Queens
      ];

      const winners = determineWinners(hands);
      expect(winners).toEqual([0]);
    });

    it('should detect multiple winners (tie)', () => {
      const hands: Card[][] = [
        [card(14, H), card(13, D), card(10, C), card(7, S), card(2, H)], // High card A
        [card(14, D), card(13, C), card(10, S), card(7, H), card(2, D)], // High card A (tie)
        [card(12, H), card(11, D), card(10, C), card(7, S), card(2, H)]  // High card Q
      ];

      const winners = determineWinners(hands);
      expect(winners).toEqual([0, 1]);
    });

    it('should handle single hand', () => {
      const hands: Card[][] = [
        [card(14, H), card(14, D), card(10, C), card(7, S), card(2, H)]
      ];

      const winners = determineWinners(hands);
      expect(winners).toEqual([0]);
    });

    it('should handle empty array', () => {
      const winners = determineWinners([]);
      expect(winners).toEqual([]);
    });
  });

  describe('getHandStrength', () => {
    it('should return higher value for better hands', () => {
      const royalFlush = getHandStrength([
        card(14, H), card(13, H), card(12, H), card(11, H), card(10, H)
      ]);
      const pair = getHandStrength([
        card(14, H), card(14, D), card(10, C), card(7, S), card(2, H)
      ]);
      const highCard = getHandStrength([
        card(14, H), card(12, D), card(10, C), card(7, S), card(2, H)
      ]);

      expect(royalFlush).toBeGreaterThan(pair);
      expect(pair).toBeGreaterThan(highCard);
    });

    it('should differentiate same hand types', () => {
      const pairOfAces = getHandStrength([
        card(14, H), card(14, D), card(10, C), card(7, S), card(2, H)
      ]);
      const pairOfKings = getHandStrength([
        card(13, H), card(13, D), card(10, C), card(7, S), card(2, H)
      ]);

      expect(pairOfAces).toBeGreaterThan(pairOfKings);
    });
  });

  describe('getHandRankName', () => {
    it('should return correct names for all hand ranks', () => {
      expect(getHandRankName(HandRank.HighCard)).toBe('High Card');
      expect(getHandRankName(HandRank.OnePair)).toBe('One Pair');
      expect(getHandRankName(HandRank.TwoPair)).toBe('Two Pair');
      expect(getHandRankName(HandRank.ThreeOfAKind)).toBe('Three of a Kind');
      expect(getHandRankName(HandRank.Straight)).toBe('Straight');
      expect(getHandRankName(HandRank.Flush)).toBe('Flush');
      expect(getHandRankName(HandRank.FullHouse)).toBe('Full House');
      expect(getHandRankName(HandRank.FourOfAKind)).toBe('Four of a Kind');
      expect(getHandRankName(HandRank.StraightFlush)).toBe('Straight Flush');
      expect(getHandRankName(HandRank.RoyalFlush)).toBe('Royal Flush');
    });
  });

  describe('Edge Cases', () => {
    it('should throw error for less than 5 cards', () => {
      expect(() => evaluateHand([
        card(14, H), card(13, H), card(12, H), card(11, H)
      ])).toThrow('Need at least 5 cards to evaluate');
    });

    it('should handle 6 cards correctly', () => {
      const cards: Card[] = [
        card(14, H), card(13, H), card(12, H), card(11, H), card(10, H), card(2, D)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.RoyalFlush);
    });

    it('should prefer higher straight flush over lower', () => {
      // Could make both 6-high and 7-high straight flush
      const cards: Card[] = [
        card(7, S), card(6, S), card(5, S), card(4, S), card(3, S), card(2, S), card(8, D)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.StraightFlush);
      expect(result.kickers[0]).toBe(7); // 7-high is best
    });

    it('should handle multiple possible flushes', () => {
      // 6 hearts - should pick best 5
      const cards: Card[] = [
        card(14, H), card(10, H), card(8, H), card(5, H), card(3, H), card(2, H), card(7, D)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.Flush);
      expect(result.kickers).toEqual([14, 10, 8, 5, 3]);
    });

    it('should not confuse almost-straight with straight', () => {
      // 9-8-7-5-4 is NOT a straight
      const cards: Card[] = [
        card(9, H), card(8, D), card(7, C), card(5, S), card(4, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.HighCard);
    });

    it('should evaluate Aces correctly as high card', () => {
      const cards: Card[] = [
        card(14, H), card(13, D), card(12, C), card(11, S), card(9, H)
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.HighCard);
      expect(result.kickers[0]).toBe(14);
    });
  });

  describe('Real Game Scenarios', () => {
    it('Scenario 1: Full board with straight possibility', () => {
      // Hole: Q♦ J♠, Board: 10♥ 9♣ 8♦ 2♥ 3♠
      // Best hand: Q-J-10-9-8 Straight
      const cards: Card[] = [
        card(12, D), card(11, S), // hole
        card(10, H), card(9, C), card(8, D), card(2, H), card(3, S) // board
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.Straight);
      expect(result.kickers[0]).toBe(12);
    });

    it('Scenario 2: Backdoor flush', () => {
      // Hole: A♠ K♠, Board: 7♠ 3♠ 2♠ Q♦ J♣
      const cards: Card[] = [
        card(14, S), card(13, S), // hole
        card(7, S), card(3, S), card(2, S), card(12, D), card(11, C) // board
      ];
      const result = evaluateHand(cards);
      expect(result.rank).toBe(HandRank.Flush);
    });

    it('Scenario 3: Counterfeited two pair', () => {
      // Hole: 7♥ 7♦, Board: A♠ A♣ K♠ K♦ 2♥
      // Board has higher two pair (AA and KK), player's 77 is useless
      const player1: Card[] = [
        card(7, H), card(7, D),
        card(14, S), card(14, C), card(13, S), card(13, D), card(2, H)
      ];
      const player2: Card[] = [
        card(12, H), card(11, D),
        card(14, S), card(14, C), card(13, S), card(13, D), card(2, H)
      ];

      const result1 = evaluateHand(player1);
      const result2 = evaluateHand(player2);

      // Both have AA KK with different kicker
      expect(result1.rank).toBe(HandRank.TwoPair);
      expect(result2.rank).toBe(HandRank.TwoPair);
      // Player 2's Q kicker beats Player 1's 7 kicker
      expect(compareHands(player1, player2)).toBe(-1);
    });

    it('Scenario 4: Split pot with identical hands', () => {
      // Both players play the board
      // Board: A♠ K♠ Q♠ J♠ 10♠ (Royal Flush on board)
      const player1: Card[] = [
        card(2, H), card(3, D),
        card(14, S), card(13, S), card(12, S), card(11, S), card(10, S)
      ];
      const player2: Card[] = [
        card(4, H), card(5, D),
        card(14, S), card(13, S), card(12, S), card(11, S), card(10, S)
      ];

      const winners = determineWinners([player1, player2]);
      expect(winners).toEqual([0, 1]); // Both win
    });
  });
});
