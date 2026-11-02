import type { Card } from '../remote/hub';

/** What a conversation is called: its name or first ask, else its tab or directory. */
export const cardTitle = (c: Card) =>
  c.title || c.tab || c.cwd.split('/').filter(Boolean).pop() || c.id;
