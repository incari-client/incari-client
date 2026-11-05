import assert from "node:assert/strict";
import { setTimeout as sleep } from "node:timers/promises";
import { afterEach, describe, it } from "node:test";

import {
  createAsyncImports,
  createPageAsyncImports,
  createPageTimer,
  type PageTimer,
  type TimerWorker,
  type WakeupScope,
} from "../../main/ts/wakeups.ts";

/** Channels the tests made: an open port keeps Node running, so each test closes them. */
const channels: MessageChannel[] = [];

class ClosableChannel extends MessageChannel {
  constructor() {
    super();
    channels.push(this);
  }
}

/** Node's own channel and timers, with a clock the test sets. */
function scope(now: () => number): WakeupScope & { timers: number } {
  const result = {
    MessageChannel: ClosableChannel,
    timers: 0,
    setTimeout(handler: () => void, delay: number): number {
      result.timers++;
      return setTimeout(handler, delay) as unknown as number;
    },
    clearTimeout(id: number): void {
      clearTimeout(id);
    },
    now,
  };
  return result;
}

/** Waits until the channel's messages have been handled. */
async function drain(): Promise<void> {
  await sleep(20);
}

describe("wakeups", () => {
  afterEach(() => {
    for (const channel of channels.splice(0)) {
      channel.port1.close();
      channel.port2.close();
    }
  });

  it("runs events due now in the order offered, without timers", async () => {
    const s = scope(() => 1000);
    const imports = createAsyncImports(s);
    const ran: string[] = [];
    for (const name of ["a", "b", "c"]) {
      imports.offer(name, (instance) => ran.push(instance as string), 1000);
    }
    assert.deepEqual(ran, []);
    await drain();
    assert.deepEqual(ran, ["a", "b", "c"]);
    assert.equal(s.timers, 0);
  });

  it("runs an event due later on a timer", async () => {
    const s = scope(() => 1000);
    const imports = createAsyncImports(s);
    const ran: string[] = [];
    const id = imports.offer("later", (instance) => ran.push(instance as string), 1005);
    assert.ok(id > 0);
    assert.equal(s.timers, 1);
    await sleep(30);
    assert.deepEqual(ran, ["later"]);
  });

  it("cancels either kind", async () => {
    const s = scope(() => 1000);
    const imports = createAsyncImports(s);
    const ran: string[] = [];
    const now = imports.offer("now", (instance) => ran.push(instance as string), 900);
    const kept = imports.offer("kept", (instance) => ran.push(instance as string), 1000);
    const later = imports.offer("later", (instance) => ran.push(instance as string), 1010);
    assert.ok(now < 0 && kept < 0);
    imports.kill(now);
    imports.kill(later);
    await sleep(40);
    assert.deepEqual(ran, ["kept"]);
    // The cancelled event's message ran nothing extra.
    imports.offer("after", (instance) => ran.push(instance as string), 1000);
    await drain();
    assert.deepEqual(ran, ["kept", "after"]);
  });

  it("keeps up with a chain of events each offering the next", async () => {
    const imports = createAsyncImports(scope(() => 0));
    let count = 0;
    const started = performance.now();
    await new Promise<void>((resolve) => {
      const step = (): void => {
        if (++count === 500) {
          resolve();
        } else {
          imports.offer(null, step, 0);
        }
      };
      imports.offer(null, step, 0);
    });
    // Timers clamped to 1 ms or more would take half a second at least.
    const elapsed = performance.now() - started;
    assert.ok(elapsed < 400, `${elapsed.toFixed(0)} ms`);
  });

  it("leaves a visible page on plain timers, and times a hidden one in the worker", async () => {
    const s = scope(() => 1000);
    let hidden = false;
    const set: number[] = [];
    const cleared: number[] = [];
    const timer: PageTimer = {
      set(handler, delay) {
        set.push(delay);
        setTimeout(handler, 1);
        return set.length;
      },
      clear(id) {
        cleared.push(id);
      },
    };
    const imports = createPageAsyncImports({ ...s, hidden: () => hidden, timer });
    const ran: string[] = [];
    const visible = imports.offer("visible", (instance) => ran.push(instance as string), 1000);
    assert.ok(visible > 0 && visible < 2 ** 30);
    assert.equal(s.timers, 1);
    hidden = true;
    const now = imports.offer("now", (instance) => ran.push(instance as string), 900);
    const later = imports.offer("later", (instance) => ran.push(instance as string), 1500);
    assert.ok(now >= 2 ** 30 && later >= 2 ** 30);
    assert.deepEqual(set, [0, 500]);
    assert.equal(s.timers, 1);
    imports.kill(later);
    assert.deepEqual(cleared, [2]);
    await sleep(30);
    assert.deepEqual([...ran].sort(), ["later", "now", "visible"]);
  });

  it("times a page timer in its worker and cancels it there", () => {
    const posted: unknown[] = [];
    const worker: TimerWorker = {
      postMessage(message) {
        posted.push(message);
      },
      onmessage: null,
    };
    const timer = createPageTimer(worker);
    const ran: number[] = [];
    const first = timer.set(() => ran.push(1), 250);
    const second = timer.set(() => ran.push(2), -5);
    timer.clear(second);
    assert.deepEqual(posted, [{ id: first, delay: 250 }, { id: second, delay: 0 }, { id: second }]);
    worker.onmessage?.({ data: second });
    worker.onmessage?.({ data: first });
    assert.deepEqual(ran, [1]);
  });
});
