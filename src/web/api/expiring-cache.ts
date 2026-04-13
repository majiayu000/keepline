export class ExpiringCache<T> {
  private entries = new Map<string, { value: T; expiresAt: number }>();

  get(key: string, now: number = Date.now()): T | null {
    const entry = this.entries.get(key);
    if (!entry) {
      return null;
    }

    if (entry.expiresAt <= now) {
      this.entries.delete(key);
      return null;
    }

    return entry.value;
  }

  set(key: string, value: T, ttlMs: number, now: number = Date.now()): T {
    this.entries.set(key, {
      value,
      expiresAt: now + ttlMs,
    });
    return value;
  }

  clear(): void {
    this.entries.clear();
  }
}
