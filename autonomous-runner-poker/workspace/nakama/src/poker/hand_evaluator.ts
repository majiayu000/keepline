/**
 * Texas Hold'em Hand Evaluator
 *
 * Evaluates poker hands and determines winners.
 * Supports all standard hand rankings from High Card to Royal Flush.
 */

import { Card, Rank, Suit, HandRank } from './types';

/**
 * Evaluated hand result
 */
export interface EvaluatedHand {
  rank: HandRank;
  cards: Card[];          // Best 5 cards
  kickers: Rank[];        // Kicker ranks for tiebreaking (high to low)
  description: string;    // Human-readable description
}

/**
 * Comparison result
 */
export type CompareResult = -1 | 0 | 1; // -1: hand1 loses, 0: tie, 1: hand1 wins

/**
 * Get all combinations of k elements from array
 */
function combinations<T>(arr: T[], k: number): T[][] {
  if (k === 0) return [[]];
  if (arr.length < k) return [];

  const result: T[][] = [];
  const first = arr[0];
  const rest = arr.slice(1);

  // Combinations including first element
  for (const combo of combinations(rest, k - 1)) {
    result.push([first, ...combo]);
  }

  // Combinations excluding first element
  result.push(...combinations(rest, k));

  return result;
}

/**
 * Sort cards by rank (descending)
 */
function sortByRank(cards: Card[]): Card[] {
  return [...cards].sort((a, b) => b.rank - a.rank);
}

/**
 * Group cards by rank
 */
function groupByRank(cards: Card[]): Map<Rank, Card[]> {
  const groups = new Map<Rank, Card[]>();
  for (const card of cards) {
    const existing = groups.get(card.rank) || [];
    existing.push(card);
    groups.set(card.rank, existing);
  }
  return groups;
}

/**
 * Group cards by suit
 */
function groupBySuit(cards: Card[]): Map<Suit, Card[]> {
  const groups = new Map<Suit, Card[]>();
  for (const card of cards) {
    const existing = groups.get(card.suit) || [];
    existing.push(card);
    groups.set(card.suit, existing);
  }
  return groups;
}

/**
 * Check if cards form a straight (5 consecutive ranks)
 * Returns the high card rank if straight, null otherwise
 * Handles A-2-3-4-5 (wheel) as a special case
 */
function checkStraight(cards: Card[]): Rank | null {
  if (cards.length < 5) return null;

  const ranks = [...new Set(cards.map(c => c.rank))].sort((a, b) => b - a);
  if (ranks.length < 5) return null;

  // Check for regular straight
  for (let i = 0; i <= ranks.length - 5; i++) {
    if (ranks[i] - ranks[i + 4] === 4) {
      return ranks[i] as Rank;
    }
  }

  // Check for wheel (A-2-3-4-5)
  if (ranks.includes(14) && ranks.includes(2) && ranks.includes(3) &&
      ranks.includes(4) && ranks.includes(5)) {
    return 5 as Rank; // 5-high straight
  }

  return null;
}

/**
 * Check for flush (5+ cards of same suit)
 * Returns the flush cards if found, null otherwise
 */
function checkFlush(cards: Card[]): Card[] | null {
  const bySuit = groupBySuit(cards);

  for (const suitCards of bySuit.values()) {
    if (suitCards.length >= 5) {
      return sortByRank(suitCards).slice(0, 5);
    }
  }

  return null;
}

/**
 * Check for straight flush
 * Returns [high rank, cards] if found, null otherwise
 */
function checkStraightFlush(cards: Card[]): [Rank, Card[]] | null {
  const bySuit = groupBySuit(cards);

  for (const suitCards of bySuit.values()) {
    if (suitCards.length >= 5) {
      const highRank = checkStraight(suitCards);
      if (highRank !== null) {
        // Get the actual straight flush cards
        const sortedSuitCards = sortByRank(suitCards);
        const straightCards: Card[] = [];

        if (highRank === 5) {
          // Wheel: A-2-3-4-5
          for (const r of [5, 4, 3, 2, 14] as Rank[]) {
            const card = sortedSuitCards.find(c => c.rank === r);
            if (card) straightCards.push(card);
          }
        } else {
          // Regular straight
          for (let r = highRank; r >= highRank - 4; r--) {
            const card = sortedSuitCards.find(c => c.rank === r);
            if (card) straightCards.push(card);
          }
        }

        return [highRank, straightCards];
      }
    }
  }

  return null;
}

/**
 * Get rank name for display
 */
function getRankName(rank: Rank): string {
  switch (rank) {
    case 14: return 'Ace';
    case 13: return 'King';
    case 12: return 'Queen';
    case 11: return 'Jack';
    default: return rank.toString();
  }
}

/**
 * Get plural rank name
 */
function getRankNamePlural(rank: Rank): string {
  switch (rank) {
    case 14: return 'Aces';
    case 13: return 'Kings';
    case 12: return 'Queens';
    case 11: return 'Jacks';
    case 6: return 'Sixes';
    default: return rank.toString() + 's';
  }
}

/**
 * Evaluate a 5-card hand
 */
function evaluate5Cards(cards: Card[]): EvaluatedHand {
  if (cards.length !== 5) {
    throw new Error('Must evaluate exactly 5 cards');
  }

  const sorted = sortByRank(cards);
  const byRank = groupByRank(cards);

  // Count occurrences of each rank
  const rankCounts = Array.from(byRank.entries())
    .map(([rank, cs]) => ({ rank, count: cs.length }))
    .sort((a, b) => b.count - a.count || b.rank - a.rank);

  // Check for flush
  const isFlush = new Set(cards.map(c => c.suit)).size === 1;

  // Check for straight
  const straightHigh = checkStraight(cards);
  const isStraight = straightHigh !== null;

  // Royal Flush: A-K-Q-J-10 of same suit
  if (isFlush && isStraight && straightHigh === 14) {
    return {
      rank: HandRank.RoyalFlush,
      cards: sorted,
      kickers: [],
      description: 'Royal Flush'
    };
  }

  // Straight Flush
  if (isFlush && isStraight) {
    return {
      rank: HandRank.StraightFlush,
      cards: sorted,
      kickers: [straightHigh],
      description: `Straight Flush, ${getRankName(straightHigh)} high`
    };
  }

  // Four of a Kind
  if (rankCounts[0].count === 4) {
    const quadRank = rankCounts[0].rank;
    const kicker = rankCounts[1].rank;
    return {
      rank: HandRank.FourOfAKind,
      cards: sorted,
      kickers: [quadRank, kicker],
      description: `Four ${getRankNamePlural(quadRank)}`
    };
  }

  // Full House
  if (rankCounts[0].count === 3 && rankCounts[1].count === 2) {
    const tripsRank = rankCounts[0].rank;
    const pairRank = rankCounts[1].rank;
    return {
      rank: HandRank.FullHouse,
      cards: sorted,
      kickers: [tripsRank, pairRank],
      description: `Full House, ${getRankNamePlural(tripsRank)} full of ${getRankNamePlural(pairRank)}`
    };
  }

  // Flush
  if (isFlush) {
    return {
      rank: HandRank.Flush,
      cards: sorted,
      kickers: sorted.map(c => c.rank),
      description: `Flush, ${getRankName(sorted[0].rank)} high`
    };
  }

  // Straight
  if (isStraight) {
    return {
      rank: HandRank.Straight,
      cards: sorted,
      kickers: [straightHigh],
      description: `Straight, ${getRankName(straightHigh)} high`
    };
  }

  // Three of a Kind
  if (rankCounts[0].count === 3) {
    const tripsRank = rankCounts[0].rank;
    const kickers = rankCounts.slice(1).map(rc => rc.rank);
    return {
      rank: HandRank.ThreeOfAKind,
      cards: sorted,
      kickers: [tripsRank, ...kickers],
      description: `Three ${getRankNamePlural(tripsRank)}`
    };
  }

  // Two Pair
  if (rankCounts[0].count === 2 && rankCounts[1].count === 2) {
    const highPair = Math.max(rankCounts[0].rank, rankCounts[1].rank) as Rank;
    const lowPair = Math.min(rankCounts[0].rank, rankCounts[1].rank) as Rank;
    const kicker = rankCounts[2].rank;
    return {
      rank: HandRank.TwoPair,
      cards: sorted,
      kickers: [highPair, lowPair, kicker],
      description: `Two Pair, ${getRankNamePlural(highPair)} and ${getRankNamePlural(lowPair)}`
    };
  }

  // One Pair
  if (rankCounts[0].count === 2) {
    const pairRank = rankCounts[0].rank;
    const kickers = rankCounts.slice(1).map(rc => rc.rank);
    return {
      rank: HandRank.OnePair,
      cards: sorted,
      kickers: [pairRank, ...kickers],
      description: `Pair of ${getRankNamePlural(pairRank)}`
    };
  }

  // High Card
  return {
    rank: HandRank.HighCard,
    cards: sorted,
    kickers: sorted.map(c => c.rank),
    description: `High Card ${getRankName(sorted[0].rank)}`
  };
}

/**
 * Evaluate the best 5-card hand from 7 cards (2 hole + 5 community)
 */
export function evaluateHand(cards: Card[]): EvaluatedHand {
  if (cards.length < 5) {
    throw new Error('Need at least 5 cards to evaluate');
  }

  if (cards.length === 5) {
    return evaluate5Cards(cards);
  }

  // Generate all 5-card combinations and find the best
  const allCombinations = combinations(cards, 5);
  let best: EvaluatedHand | null = null;

  for (const combo of allCombinations) {
    const evaluated = evaluate5Cards(combo);
    if (best === null || compareEvaluatedHands(evaluated, best) > 0) {
      best = evaluated;
    }
  }

  return best!;
}

/**
 * Compare two evaluated hands
 * Returns: 1 if hand1 wins, -1 if hand2 wins, 0 if tie
 */
export function compareEvaluatedHands(hand1: EvaluatedHand, hand2: EvaluatedHand): CompareResult {
  // Compare hand rank first
  if (hand1.rank > hand2.rank) return 1;
  if (hand1.rank < hand2.rank) return -1;

  // Same rank, compare kickers
  const maxKickers = Math.max(hand1.kickers.length, hand2.kickers.length);
  for (let i = 0; i < maxKickers; i++) {
    const k1 = hand1.kickers[i] ?? 0;
    const k2 = hand2.kickers[i] ?? 0;
    if (k1 > k2) return 1;
    if (k1 < k2) return -1;
  }

  return 0; // Tie
}

/**
 * Compare two raw hands (7 cards each)
 * Returns: 1 if hand1 wins, -1 if hand2 wins, 0 if tie
 */
export function compareHands(hand1: Card[], hand2: Card[]): CompareResult {
  const eval1 = evaluateHand(hand1);
  const eval2 = evaluateHand(hand2);
  return compareEvaluatedHands(eval1, eval2);
}

/**
 * Determine winners from multiple hands
 * Returns array of winner indices (ties result in multiple winners)
 */
export function determineWinners(hands: Card[][]): number[] {
  if (hands.length === 0) return [];
  if (hands.length === 1) return [0];

  const evaluated = hands.map(h => evaluateHand(h));

  // Find best hand
  let bestIdx = 0;
  for (let i = 1; i < evaluated.length; i++) {
    if (compareEvaluatedHands(evaluated[i], evaluated[bestIdx]) > 0) {
      bestIdx = i;
    }
  }

  // Find all hands that tie with the best
  const winners: number[] = [];
  for (let i = 0; i < evaluated.length; i++) {
    if (compareEvaluatedHands(evaluated[i], evaluated[bestIdx]) === 0) {
      winners.push(i);
    }
  }

  return winners;
}

/**
 * Get hand rank name
 */
export function getHandRankName(rank: HandRank): string {
  switch (rank) {
    case HandRank.HighCard: return 'High Card';
    case HandRank.OnePair: return 'One Pair';
    case HandRank.TwoPair: return 'Two Pair';
    case HandRank.ThreeOfAKind: return 'Three of a Kind';
    case HandRank.Straight: return 'Straight';
    case HandRank.Flush: return 'Flush';
    case HandRank.FullHouse: return 'Full House';
    case HandRank.FourOfAKind: return 'Four of a Kind';
    case HandRank.StraightFlush: return 'Straight Flush';
    case HandRank.RoyalFlush: return 'Royal Flush';
    default: return 'Unknown';
  }
}

/**
 * Calculate hand strength as a numeric value for comparison
 * Higher value = stronger hand
 * Useful for quick comparisons
 */
export function getHandStrength(cards: Card[]): number {
  const hand = evaluateHand(cards);

  // Base score from rank (multiplied by large number to ensure rank is primary)
  let score = hand.rank * 100000000;

  // Add kicker values
  for (let i = 0; i < hand.kickers.length; i++) {
    score += hand.kickers[i] * Math.pow(15, 4 - i);
  }

  return score;
}
