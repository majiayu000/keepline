import type { AgentClient } from '../../domain/session/index.js';

// Receipt time belongs to this receiver process, not to the event's authored timestamp.
const lastReceived = new Map<AgentClient, string>();
export function recordHookReceipt(client: AgentClient) {
  lastReceived.set(client, new Date().toISOString());
}
export function lastHookReceipt(client: AgentClient) {
  return lastReceived.get(client);
}
