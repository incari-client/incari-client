/**
 * TeaVM's event queue in the browser (`teavmAsync`), without the browser's timer clamp. TeaVM hands
 * every event to `setTimeout`, a woken thread too (a `notify`, a finished wait): due at once, so a
 * zero delay. But browsers hold a zero-delay timer set from inside a timer's callback, five deep, to
 * at least 4 ms, and one thread waking another is just such a chain. In singleplayer's server worker
 * each chunk storage scan (the game's structure check) is a hand-off of that kind between its IO
 * thread and the world generation waiting on it, so a new world's ~8,000 scans took 16-22 seconds
 * of timers, with the worker idle.
 *
 * Here an event due now goes on a queue that a `MessageChannel` drains, one event per message: the
 * order TeaVM gave them, and no clamp (the background pool's slots use the same, `WorkerSlots`). An
 * event due later still waits on `setTimeout`. Ids for queued events are negative, so `kill` knows
 * which kind it cancels.
 */

/** The `teavmAsync` import module: `offer` returns an id `kill` takes back. */
export interface AsyncImports {
  offer(instance: unknown, fn: (instance: unknown) => void, time: number): number;
  kill(id: number): void;
}

/** What the imports need from the global scope, so tests can supply their own. */
export interface WakeupScope {
  MessageChannel: new () => MessageChannel;
  setTimeout(handler: () => void, delay: number): number;
  clearTimeout(id: number): void;
  now(): number;
}

const browserScope: WakeupScope = {
  MessageChannel,
  setTimeout: (handler, delay) => setTimeout(handler, delay) as unknown as number,
  clearTimeout: (id) => {
    clearTimeout(id);
  },
  now: () => Date.now(),
};

export function createAsyncImports(scope: WakeupScope = browserScope): AsyncImports {
  const channel = new scope.MessageChannel();
  // Insertion order is the order events were offered in.
  const due = new Map<number, () => void>();
  let nextId = 1;
  channel.port1.onmessage = () => {
    const first = due.entries().next();
    if (first.done !== true) {
      const [id, run] = first.value;
      due.delete(id);
      run();
    }
  };
  return {
    offer(instance, fn, time) {
      const delay = time - scope.now();
      if (delay > 0) {
        return scope.setTimeout(() => {
          fn(instance);
        }, delay);
      }
      const id = -nextId++;
      due.set(id, () => {
        fn(instance);
      });
      channel.port2.postMessage(0);
      return id;
    },
    kill(id) {
      if (id < 0) {
        // Its message still comes; it runs the next event due, or nothing.
        due.delete(id);
      } else {
        scope.clearTimeout(id);
      }
    },
  };
}

/**
 * A timer whose callbacks a hidden page still gets on time: browsers hold a hidden page's own timers
 * to one wake-up a second, and after five minutes hidden and silent (Chrome) to one a minute, which
 * starves a server connection of its keep-alive answers until the server drops it. A worker's timer
 * isn't held back that way, and its message wakes the page.
 */
export interface PageTimer {
  set(handler: () => void, delay: number): number;
  clear(id: number): void;
}

/** The parts of a `Worker` the timer uses, so tests can supply their own. */
export interface TimerWorker {
  postMessage(message: unknown): void;
  onmessage: ((event: { data: unknown }) => void) | null;
}

/** The timer worker's whole program: a timeout per request, answered with its id. */
const TIMER_WORKER_SOURCE =
  "var timers = new Map();" +
  "onmessage = function (e) { var m = e.data;" +
  " if (m.delay !== undefined) { timers.set(m.id, setTimeout(function () { timers.delete(m.id); postMessage(m.id); }, m.delay)); }" +
  " else { var t = timers.get(m.id); if (t !== undefined) { clearTimeout(t); timers.delete(m.id); } } };";

/** Starts the timer worker from a Blob URL; null where the page can't have one. */
export function startTimerWorker(): TimerWorker | null {
  try {
    const url = URL.createObjectURL(new Blob([TIMER_WORKER_SOURCE], { type: "text/javascript" }));
    const worker = new Worker(url, { name: "incari-timer" });
    const timerWorker: TimerWorker = {
      postMessage: (message) => {
        worker.postMessage(message);
      },
      onmessage: null,
    };
    worker.onmessage = (event: MessageEvent<unknown>) => {
      timerWorker.onmessage?.({ data: event.data });
    };
    return timerWorker;
  } catch {
    return null;
  }
}

/** A {@link PageTimer} over a timer worker. */
export function createPageTimer(worker: TimerWorker): PageTimer {
  const handlers = new Map<number, () => void>();
  let nextId = 1;
  worker.onmessage = (event) => {
    const id = Number(event.data);
    const handler = handlers.get(id);
    if (handler !== undefined) {
      handlers.delete(id);
      handler();
    }
  };
  return {
    set(handler, delay) {
      const id = nextId++;
      handlers.set(id, handler);
      worker.postMessage({ id, delay: Math.max(0, delay) });
      return id;
    },
    clear(id) {
      if (handlers.delete(id)) {
        worker.postMessage({ id });
      }
    },
  };
}

/** What the page's imports need: timers, whether the page is hidden, and the worker timer. */
export interface PageWakeupScope extends Omit<WakeupScope, "MessageChannel"> {
  hidden(): boolean;
  timer: PageTimer | null;
}

/** Ids of events on the {@link PageTimer}: above any id the page's own timers give out. */
const PAGE_TIMER_IDS = 2 ** 30;

/**
 * The page's `teavmAsync`: TeaVM's own (every event on `setTimeout`) while the page is visible, so
 * a visible page runs exactly as it always has; while it's hidden, every event goes on the
 * {@link PageTimer}, which the browser doesn't hold back. So a hidden page's connection threads still
 * answer the server, and its frames still come. Not the `MessageChannel` of {@link createAsyncImports}:
 * on the page it once hung singleplayer at spawn (ADR 0043), and a spinning wait would busy a hidden
 * tab's core, where a worker's round trip paces it.
 */
export function createPageAsyncImports(scope: PageWakeupScope): AsyncImports {
  return {
    offer(instance, fn, time) {
      const delay = Math.max(0, time - scope.now());
      const timer = scope.timer;
      if (scope.hidden() && timer !== null) {
        return (
          PAGE_TIMER_IDS +
          timer.set(() => {
            fn(instance);
          }, delay)
        );
      }
      return scope.setTimeout(() => {
        fn(instance);
      }, delay);
    },
    kill(id) {
      if (id >= PAGE_TIMER_IDS) {
        scope.timer?.clear(id - PAGE_TIMER_IDS);
      } else {
        scope.clearTimeout(id);
      }
    },
  };
}

/** The page's {@link PageWakeupScope}: the browser's own channel and timers, and `document.hidden`. */
export function pageWakeupScope(timer: PageTimer | null): PageWakeupScope {
  return { ...browserScope, hidden: () => document.hidden, timer };
}
