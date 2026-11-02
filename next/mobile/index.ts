// Hermes has no crypto.getRandomValues; the Noise handshake's ephemeral keys need it.
// Set it up before anything imports @noble, then hand over to Expo Router.
import * as Crypto from 'expo-crypto';

const g = globalThis as { crypto?: { getRandomValues?: unknown } };
if (typeof g.crypto?.getRandomValues !== 'function') {
  g.crypto = { ...(g.crypto ?? {}), getRandomValues: Crypto.getRandomValues };
}

require('expo-router/entry');
