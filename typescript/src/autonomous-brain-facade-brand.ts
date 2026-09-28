const autonomousBrainFacades = new WeakSet<object>();

export function registerAutonomousBrainFacade(value: object): void {
  autonomousBrainFacades.add(value);
}

export function isAutonomousBrainFacade(value: unknown): value is object {
  return typeof value === "object" && value !== null && autonomousBrainFacades.has(value);
}
