import { ProtocolError } from "./errors.js";
import type { SseEvent } from "./types.js";

/** Parse one bounded SSE snapshot according to the field and blank-line rules of EventSource. */
export function parseSse(input: string): SseEvent[] {
  if (typeof input !== "string") throw new ProtocolError("SSE response must be text");
  const events: SseEvent[] = [];
  let current: Partial<SseEvent> & { dataLines?: string[] } = { dataLines: [] };
  const lines = input.replaceAll("\r\n", "\n").replaceAll("\r", "\n").split("\n");

  const dispatch = (): void => {
    const dataLines = current.dataLines ?? [];
    if (dataLines.length > 0) {
      const event: SseEvent = { data: dataLines.join("\n") };
      if (current.id !== undefined) event.id = current.id;
      if (current.event !== undefined) event.event = current.event;
      if (current.retry !== undefined) event.retry = current.retry;
      events.push(event);
    }
    current = { dataLines: [] };
  };

  for (const line of lines) {
    if (line === "") {
      dispatch();
      continue;
    }
    if (line.startsWith(":")) continue;
    const separator = line.indexOf(":");
    const field = separator < 0 ? line : line.slice(0, separator);
    const valueStart = separator < 0 ? line.length : separator + 1;
    const value = line[valueStart] === " " ? line.slice(valueStart + 1) : line.slice(valueStart);
    switch (field) {
      case "id":
        if (value.includes("\u0000")) throw new ProtocolError("SSE id contains a NUL character");
        current.id = value;
        break;
      case "event":
        current.event = value;
        break;
      case "data":
        current.dataLines?.push(value);
        break;
      case "retry":
        if (!/^\d+$/.test(value)) throw new ProtocolError("SSE retry is not an unsigned integer");
        current.retry = Number(value);
        if (!Number.isSafeInteger(current.retry)) throw new ProtocolError("SSE retry exceeds safe integer range");
        break;
      default:
        // The gateway may add extension fields. EventSource ignores unknown fields by design.
        break;
    }
  }
  dispatch();
  return events;
}
