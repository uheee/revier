export class SessionCache<TValue> {
  private readonly values = new Map<string, TValue>();

  get(key: string): TValue | undefined {
    return this.values.get(key);
  }

  set(key: string, value: TValue): void {
    this.values.set(key, value);
  }

  delete(key: string): void {
    this.values.delete(key);
  }

  clear(): void {
    this.values.clear();
  }
}
