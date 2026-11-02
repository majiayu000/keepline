// Paired machines, each its pairing link. The link holds this device's key, so it lives
// in the Keychain / Keystore (expo-secure-store), one entry per machine to stay under the
// ~2 KB some iOS versions accept per value. On web, used only to try the UI in a browser,
// it falls back to localStorage.
import * as SecureStore from 'expo-secure-store';
import { Platform } from 'react-native';
import { type Link, linkText, parseLink } from '../remote/link';

const INDEX = 'keepline.machines';
const entry = (id: string) => `keepline.machine.${id.replace(/[^A-Za-z0-9._-]/g, '_')}`;

const store =
  Platform.OS === 'web'
    ? {
        get: async (k: string) => globalThis.localStorage?.getItem(k) ?? null,
        set: async (k: string, v: string) => globalThis.localStorage?.setItem(k, v),
        del: async (k: string) => globalThis.localStorage?.removeItem(k),
      }
    : {
        get: (k: string) => SecureStore.getItemAsync(k),
        set: (k: string, v: string) => SecureStore.setItemAsync(k, v),
        del: (k: string) => SecureStore.deleteItemAsync(k),
      };

/** A machine is known by its id; a link without one (older machines) by its device. */
export const machineId = (l: Link) => l.machine || `device-${l.device}`;

async function ids(): Promise<string[]> {
  try {
    return JSON.parse((await store.get(INDEX)) ?? '[]');
  } catch {
    return [];
  }
}

export async function loadMachines(): Promise<Link[]> {
  const out: Link[] = [];
  for (const id of await ids()) {
    const text = await store.get(entry(id));
    if (!text) continue;
    try {
      out.push(parseLink(text));
    } catch {
      // A link this version cannot read is skipped, not deleted.
    }
  }
  return out;
}

/** Pairing the same machine again replaces its old link. */
export async function saveMachine(link: Link): Promise<void> {
  const id = machineId(link);
  await store.set(entry(id), linkText(link));
  const all = await ids();
  if (!all.includes(id)) await store.set(INDEX, JSON.stringify([...all, id]));
}

export async function forgetMachine(id: string): Promise<void> {
  await store.del(entry(id));
  await store.set(INDEX, JSON.stringify((await ids()).filter((x) => x !== id)));
}
