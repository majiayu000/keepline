/**
 * RPC functions for finding and creating poker matches
 */

// Module name must match the registered match handler
const POKER_MATCH_MODULE = 'poker';

interface FindMatchRequest {
  minPlayers?: number;
  maxPlayers?: number;
  blinds?: string; // e.g., "10/20"
}

interface FindMatchResponse {
  matchId: string;
  label: string;
  created: boolean;
}

interface CreatePrivateMatchRequest {
  label?: string;
  minPlayers?: number;
  maxPlayers?: number;
  smallBlind?: number;
  bigBlind?: number;
  startingChips?: number;
}

/**
 * Find an available match or create a new one
 */
export const findMatchRpc: nkruntime.RpcFunction = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  payload: string
): string {

  logger.info('Find match RPC called', { userId: ctx.userId, payload });

  let request: FindMatchRequest = {};
  if (payload && payload.length > 0) {
    try {
      request = JSON.parse(payload);
    } catch (e) {
      logger.error('Failed to parse payload', { error: e });
    }
  }

  // Query parameters for match listing
  const limit = 10;
  const authoritative = true;
  const label = ''; // Empty to match any label
  const minSize = 0;
  const maxSize = (request.maxPlayers || 9) - 1; // Look for matches with space

  // List existing matches
  const matches = nk.matchList(limit, authoritative, label, minSize, maxSize, '');

  // Try to find a suitable match
  for (const match of matches) {
    try {
      const matchLabel = JSON.parse(match.label || '{}');

      // Check if blinds match (if specified)
      if (request.blinds && matchLabel.blinds !== request.blinds) {
        continue;
      }

      // Check if match has space
      if (matchLabel.players < matchLabel.maxPlayers) {
        logger.info('Found existing match', { matchId: match.matchId });

        const response: FindMatchResponse = {
          matchId: match.matchId,
          label: match.label || '',
          created: false
        };

        return JSON.stringify(response);
      }
    } catch (e) {
      logger.warn('Failed to parse match label', { matchId: match.matchId, error: e });
    }
  }

  // No suitable match found, create a new one
  logger.info('No suitable match found, creating new one');

  const params: { [key: string]: string } = {
    label: 'Texas Hold\'em'
  };

  if (request.minPlayers) {
    params.minPlayers = request.minPlayers.toString();
  }
  if (request.maxPlayers) {
    params.maxPlayers = request.maxPlayers.toString();
  }
  if (request.blinds) {
    const [small, big] = request.blinds.split('/');
    params.smallBlind = small;
    params.bigBlind = big;
  }

  const matchId = nk.matchCreate(POKER_MATCH_MODULE, params);

  logger.info('Created new match', { matchId });

  const response: FindMatchResponse = {
    matchId: matchId,
    label: params.label,
    created: true
  };

  return JSON.stringify(response);
};

/**
 * Room info for listing
 */
interface RoomInfo {
  matchId: string;
  label: string;
  players: number;
  maxPlayers: number;
  spectators: number;
  blinds: string;
  phase: string;
  createdAt?: number;
}

interface ListRoomsResponse {
  rooms: RoomInfo[];
  total: number;
}

/**
 * List all available poker rooms
 */
export const listRoomsRpc: nkruntime.RpcFunction = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  payload: string
): string {

  logger.info('List rooms RPC called', { userId: ctx.userId });

  const limit = 50;
  const authoritative = true;
  const label = '';
  const minSize = 0;
  const maxSize = 9; // Max players per table

  const matches = nk.matchList(limit, authoritative, label, minSize, maxSize, '');

  const rooms: RoomInfo[] = [];

  for (const match of matches) {
    try {
      const matchLabel = JSON.parse(match.label || '{}');

      rooms.push({
        matchId: match.matchId,
        label: matchLabel.name || 'Texas Hold\'em',
        players: matchLabel.players || match.size,
        maxPlayers: matchLabel.maxPlayers || 9,
        spectators: matchLabel.spectators || 0,
        blinds: matchLabel.blinds || '10/20',
        phase: matchLabel.phase || 'waiting',
        createdAt: matchLabel.createdAt,
      });
    } catch (e) {
      logger.warn('Failed to parse match label', { matchId: match.matchId, error: e });
      // Include match with default values
      rooms.push({
        matchId: match.matchId,
        label: 'Texas Hold\'em',
        players: match.size,
        maxPlayers: 9,
        spectators: 0,
        blinds: '10/20',
        phase: 'unknown',
      });
    }
  }

  // Sort by player count (most players first), then by creation time
  rooms.sort((a, b) => {
    if (b.players !== a.players) {
      return b.players - a.players;
    }
    return (b.createdAt || 0) - (a.createdAt || 0);
  });

  const response: ListRoomsResponse = {
    rooms,
    total: rooms.length,
  };

  logger.info('Listed rooms', { count: rooms.length });

  return JSON.stringify(response);
};

/**
 * Create a private match with custom settings
 */
export const createPrivateMatchRpc: nkruntime.RpcFunction = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  payload: string
): string {

  logger.info('Create private match RPC called', { userId: ctx.userId, payload });

  let request: CreatePrivateMatchRequest = {};
  if (payload && payload.length > 0) {
    try {
      request = JSON.parse(payload);
    } catch (e) {
      logger.error('Failed to parse payload', { error: e });
    }
  }

  // Build match parameters
  const params: { [key: string]: string } = {
    label: request.label || 'Private Game'
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

  // Create the match
  const matchId = nk.matchCreate(POKER_MATCH_MODULE, params);

  logger.info('Created private match', { matchId, params });

  return JSON.stringify({
    matchId: matchId,
    label: params.label
  });
};
