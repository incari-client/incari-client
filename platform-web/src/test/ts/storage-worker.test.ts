import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { describe, it } from "node:test";
import { runInThisContext } from "node:vm";

import { IDBFactory, IDBKeyRange } from "fake-indexeddb";

// The storage worker as the page runs it: its source, run against a stand-in for the worker's
// global scope, driven through the same messages the page sends.
const source = readFileSync(new URL("../../main/js/storage-worker.js", import.meta.url), "utf8");

type Message = Record<string, unknown>;

function domError(name: string, message: string): Error {
  const error = new Error(message);
  error.name = name;
  return error;
}

/** An origin private file system in memory, as much of it as the worker uses. */
class FakeFile {
  readonly kind = "file";
  data = new Uint8Array(0);
  lastModified = 1;
  openHandles = 0;
  readonly withSyncHandles: boolean;

  constructor(withSyncHandles: boolean) {
    this.withSyncHandles = withSyncHandles;
  }

  reads = 0;

  getFile(): Promise<{ lastModified: number; size: number; arrayBuffer(): Promise<ArrayBuffer> }> {
    const copy = this.data.slice();
    const lastModified = this.lastModified;
    return Promise.resolve({
      lastModified,
      size: copy.byteLength,
      arrayBuffer: () => {
        this.reads++;
        return Promise.resolve(copy.buffer);
      },
    });
  }

  get createSyncAccessHandle(): (() => Promise<unknown>) | undefined {
    if (!this.withSyncHandles) {
      return undefined;
    }
    return () => {
      if (this.openHandles > 0) {
        return Promise.reject(domError("NoModificationAllowedError", "already open"));
      }
      this.openHandles++;
      return Promise.resolve({
        truncate: (size: number) => {
          const next = new Uint8Array(size);
          next.set(this.data.subarray(0, Math.min(size, this.data.length)));
          this.data = next;
        },
        write: (data: Uint8Array, options: { at: number }) => {
          const end = options.at + data.byteLength;
          if (end > this.data.length) {
            const next = new Uint8Array(end);
            next.set(this.data);
            this.data = next;
          }
          this.data.set(data, options.at);
          this.lastModified++;
          return data.byteLength;
        },
        flush: () => undefined,
        close: () => {
          this.openHandles--;
        },
      });
    };
  }

  get createWritable(): (() => Promise<unknown>) | undefined {
    if (this.withSyncHandles) {
      return undefined;
    }
    return () => {
      let copy = this.data.slice();
      return Promise.resolve({
        truncate: (size: number) => {
          const next = new Uint8Array(size);
          next.set(copy.subarray(0, Math.min(size, copy.length)));
          copy = next;
          return Promise.resolve();
        },
        write: (params: { position: number; data: Uint8Array }) => {
          const end = params.position + params.data.byteLength;
          if (end > copy.length) {
            const next = new Uint8Array(end);
            next.set(copy);
            copy = next;
          }
          copy.set(params.data, params.position);
          return Promise.resolve();
        },
        close: () => {
          this.data = copy;
          return Promise.resolve();
        },
      });
    };
  }
}

class FakeDirectory {
  readonly kind = "directory";
  readonly children = new Map<string, FakeDirectory | FakeFile>();
  readonly withSyncHandles: boolean;

  constructor(withSyncHandles = true) {
    this.withSyncHandles = withSyncHandles;
  }

  getDirectoryHandle(name: string, options?: { create?: boolean }): Promise<FakeDirectory> {
    const child = this.children.get(name);
    if (child instanceof FakeDirectory) {
      return Promise.resolve(child);
    }
    if (child) {
      return Promise.reject(domError("TypeMismatchError", `${name} is a file`));
    }
    if (!options?.create) {
      return Promise.reject(domError("NotFoundError", `no ${name}`));
    }
    const made = new FakeDirectory(this.withSyncHandles);
    this.children.set(name, made);
    return Promise.resolve(made);
  }

  getFileHandle(name: string, options?: { create?: boolean }): Promise<FakeFile> {
    const child = this.children.get(name);
    if (child instanceof FakeFile) {
      return Promise.resolve(child);
    }
    if (child) {
      return Promise.reject(domError("TypeMismatchError", `${name} is a folder`));
    }
    if (!options?.create) {
      return Promise.reject(domError("NotFoundError", `no ${name}`));
    }
    const made = new FakeFile(this.withSyncHandles);
    this.children.set(name, made);
    return Promise.resolve(made);
  }

  removeEntry(name: string, options?: { recursive?: boolean }): Promise<void> {
    const child = this.children.get(name);
    if (!child) {
      return Promise.reject(domError("NotFoundError", `no ${name}`));
    }
    if (child instanceof FakeDirectory && child.children.size > 0 && !options?.recursive) {
      return Promise.reject(domError("InvalidModificationError", `${name} isn't empty`));
    }
    this.children.delete(name);
    return Promise.resolve();
  }

  async *entries(): AsyncGenerator<[string, FakeDirectory | FakeFile]> {
    for (const entry of [...this.children.entries()]) {
      await Promise.resolve();
      yield entry;
    }
  }
}

/** The text of the file at a path in a stand-in file system, or undefined. */
function textAt(root: FakeDirectory, path: string): string | undefined {
  const names = path.split("/").filter((part) => part.length > 0);
  let node: FakeDirectory | FakeFile | undefined = root;
  for (const name of names) {
    node = node instanceof FakeDirectory ? node.children.get(name) : undefined;
  }
  return node instanceof FakeFile ? new TextDecoder().decode(node.data) : undefined;
}

interface Options {
  opfs?: FakeDirectory | null;
  indexedDB?: IDBFactory;
  lockTaken?: boolean;
  fetch?: (url: string) => Promise<Response>;
}

/** A worker started on a stand-in scope, and a way to talk to it. */
class Worker {
  readonly posted: Message[] = [];
  private readonly waiting: { match: (message: Message) => boolean; resolve: (message: Message) => void }[] = [];
  private onmessage: ((event: { data: Message }) => void) | null = null;
  private next = 1;

  constructor(options: Options) {
    const listen = (handler: ((event: { data: Message }) => void) | null) => {
      this.onmessage = handler;
    };
    const scope = {
      postMessage: (message: Message) => {
        this.posted.push(message);
        const index = this.waiting.findIndex((wait) => wait.match(message));
        if (index >= 0) {
          const [wait] = this.waiting.splice(index, 1);
          wait?.resolve(message);
        }
      },
      set onmessage(handler: ((event: { data: Message }) => void) | null) {
        listen(handler);
      },
      navigator: {
        storage: {
          getDirectory: () =>
            options.opfs ? Promise.resolve(options.opfs) : Promise.reject(domError("SecurityError", "no OPFS here")),
        },
        locks: {
          request: (_name: string, _options: unknown, callback: (lock: object | null) => unknown) => {
            void callback(options.lockTaken ? null : {});
            return Promise.resolve();
          },
        },
      },
      indexedDB: options.indexedDB,
      IDBKeyRange,
      fetch: options.fetch ?? (() => Promise.reject(new Error("no network in this test"))),
    };
    const start = runInThisContext(`(function(globalThis) {\n${source}\n})`) as (scope: unknown) => void;
    start(scope);
  }

  send(message: Message): void {
    this.onmessage?.({ data: message });
  }

  /** Sends a message with a new id and waits for the answer with that id. */
  request(message: Message): Promise<Message> {
    const id = this.next++;
    const answer = this.waitFor((reply) => reply.id === id);
    this.send({ ...message, id });
    return answer;
  }

  /** Sends a batch of changes and waits until they're stored. */
  apply(ops: Message[]): Promise<Message> {
    const done = this.waitFor((reply) => reply.type === "applied");
    this.send({ type: "apply", ops });
    return done;
  }

  waitFor(match: (message: Message) => boolean): Promise<Message> {
    return new Promise((resolve) => this.waiting.push({ match, resolve }));
  }

  open(inPlace?: { within: string; suffixes: string[] }): Promise<Message> {
    return this.request({ type: "open", root: "/incari", lock: "incari-storage", inPlace });
  }
}

/**
 * One end of a message channel, as the worker sees a port: what it posts is recorded, and what's
 * sent to it before it has a handler waits for one, as a real port's messages do.
 */
class FakePort {
  readonly posted: Message[] = [];
  private handler: ((event: { data: Message }) => void) | null = null;
  private readonly early: Message[] = [];
  private readonly waiting: { match: (message: Message) => boolean; resolve: (message: Message) => void }[] = [];

  get onmessage(): ((event: { data: Message }) => void) | null {
    return this.handler;
  }

  set onmessage(handler: ((event: { data: Message }) => void) | null) {
    this.handler = handler;
    for (const message of this.early.splice(0)) {
      handler?.({ data: message });
    }
  }

  postMessage(message: Message): void {
    this.posted.push(message);
    const index = this.waiting.findIndex((wait) => wait.match(message));
    if (index >= 0) {
      const [wait] = this.waiting.splice(index, 1);
      wait?.resolve(message);
    }
  }

  /** Sends the worker a message on this port, and waits for an answer that matches. */
  ask(message: Message, match: (reply: Message) => boolean): Promise<Message> {
    const answer = new Promise<Message>((resolve) => this.waiting.push({ match, resolve }));
    if (this.handler) {
      this.handler({ data: message });
    } else {
      this.early.push(message);
    }
    return answer;
  }
}

/** The stand-in file at a path, or undefined. */
function fileAt(root: FakeDirectory, path: string): FakeFile | undefined {
  const names = path.split("/").filter((part) => part.length > 0);
  let node: FakeDirectory | FakeFile | undefined = root;
  for (const name of names) {
    node = node instanceof FakeDirectory ? node.children.get(name) : undefined;
  }
  return node instanceof FakeFile ? node : undefined;
}

function bytes(text: string): Uint8Array {
  return new TextEncoder().encode(text);
}

interface Entry {
  path: string;
  dir: boolean;
  data: ArrayBuffer | null;
}

/** The entries an "open" answered, as "path" for folders and "path=text" for files, sorted. */
function listing(reply: Message): string[] {
  const entries = reply.entries as Entry[];
  return entries
    .map((entry) => (entry.dir ? entry.path : `${entry.path}=${new TextDecoder().decode(entry.data ?? undefined)}`))
    .sort();
}

const changes: Message[] = [
  { op: "mkdir", path: "/incari/game", mtime: 1 },
  { op: "write", path: "/incari/game/options.txt", size: 7, at: 0, data: bytes("fov:0.5"), mtime: 2 },
  { op: "write", path: "/incari/game/servers.dat", size: 3, at: 0, data: bytes("abc"), mtime: 3 },
  { op: "mkdir", path: "/incari/game/resourcepacks", mtime: 4 },
  { op: "write", path: "/incari/game/resourcepacks/p.zip", size: 2, at: 0, data: bytes("PK"), mtime: 5 },
  // Part of a file: bytes 1-2 of servers.dat, which also grows to 5 bytes.
  { op: "write", path: "/incari/game/servers.dat", size: 5, at: 1, data: bytes("XY"), mtime: 6 },
  { op: "delete", path: "/incari/game/resourcepacks" },
];

const afterChanges = ["/incari/game", "/incari/game/options.txt=fov:0.5", "/incari/game/servers.dat=aXY\0\0"];

describe("the storage worker", () => {
  for (const withSyncHandles of [true, false]) {
    it(`keeps files in the origin private file system (${withSyncHandles ? "sync handles" : "writable streams"})`, async () => {
      const opfs = new FakeDirectory(withSyncHandles);
      const first = new Worker({ opfs });
      const opened = await first.open();
      assert.equal(opened.backend, "opfs");
      assert.equal(opened.readOnly, false);
      assert.deepEqual(opened.entries, []);
      const applied = await first.apply(changes);
      assert.equal(applied.error, undefined);
      assert.equal(textAt(opfs, "/incari/game/options.txt"), "fov:0.5");

      // The next page load.
      const second = new Worker({ opfs });
      assert.deepEqual(listing(await second.open()), afterChanges);
      const read = await second.request({ type: "read", path: "/incari/game/options.txt" });
      assert.equal(new TextDecoder().decode(read.data as ArrayBuffer), "fov:0.5");
      assert.equal((await second.request({ type: "read", path: "/incari/nothing" })).data, null);
      assert.deepEqual((await second.request({ type: "list", path: "/incari/game" })).paths, [
        "/incari/game/options.txt",
        "/incari/game/servers.dat",
      ]);
    });
  }

  it("lists in-place files with their sizes and leaves their bytes where they are", async () => {
    const opfs = new FakeDirectory();
    const first = new Worker({ opfs });
    await first.open();
    const region = "/incari/game/saves/World/region/r.0.0.mca";
    await first.apply([
      { op: "write", path: region, size: 5, at: 0, data: bytes("chunk"), mtime: 1 },
      { op: "write", path: "/incari/game/saves/World/level.dat", size: 1, at: 0, data: bytes("L"), mtime: 1 },
      { op: "write", path: "/incari/game/r.0.0.mca", size: 3, at: 0, data: bytes("not"), mtime: 1 },
    ]);
    const reads = fileAt(opfs, region)?.reads ?? -1;

    const second = new Worker({ opfs });
    const opened = await second.open({ within: "/saves/", suffixes: [".mca", ".mcc"] });
    const entries = opened.entries as (Entry & { size?: number; stored?: boolean })[];
    const stored = entries.find((entry) => entry.path === region);
    assert.deepEqual([stored?.stored, stored?.size, stored?.data], [true, 5, null]);
    assert.equal(fileAt(opfs, region)?.reads, reads);
    // Only inside a saves folder.
    assert.equal(entries.find((entry) => entry.path === "/incari/game/r.0.0.mca")?.stored, undefined);
    assert.ok(listing(opened).includes("/incari/game/saves/World/level.dat=L"));

    const stat = await second.request({ type: "stat", path: "/incari/game/saves/World" });
    const sizes = (stat.entries as { path: string; size?: number }[])
      .filter((entry) => entry.size !== undefined)
      .map((entry) => `${entry.path}:${String(entry.size)}`)
      .sort();
    assert.deepEqual(sizes, ["/incari/game/saves/World/level.dat:1", `${region}:5`]);
  });

  it("stores the rest of a batch when one change fails, and names the one that did", async () => {
    const opfs = new FakeDirectory();
    const worker = new Worker({ opfs });
    await worker.open();
    await worker.apply([{ op: "write", path: "/incari/held", size: 1, at: 0, data: bytes("h"), mtime: 1 }]);
    const held = (opfs.children.get("incari") as FakeDirectory).children.get("held") as FakeFile;
    held.openHandles = 1;
    const applied = await worker.apply([
      { op: "write", path: "/incari/held", size: 2, at: 1, data: bytes("2"), mtime: 2 },
      { op: "write", path: "/incari/fine", size: 1, at: 0, data: bytes("f"), mtime: 2 },
    ]);
    assert.match(String(applied.error), /NoModificationAllowedError/);
    assert.deepEqual(applied.failed, ["/incari/held"]);
    assert.equal(textAt(opfs, "/incari/fine"), "f");
    assert.equal(textAt(opfs, "/incari/held"), "h");
  });

  it("writes a file's ranges through one handle, in order", async () => {
    const opfs = new FakeDirectory();
    const worker = new Worker({ opfs });
    await worker.open();
    await worker.apply([{ op: "write", path: "/incari/r.mca", size: 10, at: 0, data: bytes("0123456789"), mtime: 1 }]);
    const applied = await worker.apply([
      { op: "write", path: "/incari/r.mca", size: 12, at: 1, data: bytes("ab"), mtime: 2 },
      { op: "write", path: "/incari/r.mca", size: 12, at: 9, data: bytes("XYZ"), mtime: 2 },
      { op: "write", path: "/incari/other", size: 1, at: 0, data: bytes("o"), mtime: 2 },
    ]);
    assert.equal(applied.error, undefined);
    assert.equal(textAt(opfs, "/incari/r.mca"), "0ab345678XYZ");
    assert.equal(textAt(opfs, "/incari/other"), "o");
  });

  it("takes changes from a port, in the page's queue, and answers there", async () => {
    const opfs = new FakeDirectory();
    const worker = new Worker({ opfs });
    await worker.open();
    const port = new FakePort();
    worker.send({ type: "port", port });
    const applied = await port.ask(
      {
        type: "apply",
        ops: [{ op: "write", path: "/incari/game/saves/W/level.dat", size: 1, at: 0, data: bytes("1"), mtime: 1 }],
      },
      (reply) => reply.type === "applied",
    );
    assert.equal(applied.error, undefined);
    assert.equal(textAt(opfs, "/incari/game/saves/W/level.dat"), "1");
    const synced = await port.ask({ type: "sync", id: 7 }, (reply) => reply.id === 7);
    assert.equal(synced.error, undefined);
    // The page's own requests still answer the page.
    assert.equal((await worker.request({ type: "sync" })).error, undefined);
    assert.equal(port.posted.length, 2);
  });

  it("replaces a file with a folder and a folder with a file", async () => {
    const opfs = new FakeDirectory();
    const worker = new Worker({ opfs });
    await worker.open();
    await worker.apply([
      { op: "write", path: "/incari/a", size: 1, at: 0, data: bytes("1"), mtime: 1 },
      { op: "mkdir", path: "/incari/b/c", mtime: 1 },
    ]);
    await worker.apply([
      { op: "mkdir", path: "/incari/a", mtime: 2 },
      { op: "write", path: "/incari/b", size: 1, at: 0, data: bytes("2"), mtime: 2 },
      { op: "delete", path: "/incari/never-there" },
    ]);
    assert.deepEqual(listing(await new Worker({ opfs }).open()), ["/incari/a", "/incari/b=2"]);
  });

  it("falls back to IndexedDB where there's no origin private file system", async () => {
    const indexedDB = new IDBFactory();
    const first = new Worker({ opfs: null, indexedDB });
    const opened = await first.open();
    assert.equal(opened.backend, "indexeddb");
    assert.match(String(opened.problems), /no origin private file system/);
    const applied = await first.apply(changes);
    assert.equal(applied.error, undefined);
    const second = new Worker({ opfs: null, indexedDB });
    assert.deepEqual(listing(await second.open()), afterChanges);
  });

  it("keeps nothing when the browser offers no storage", async () => {
    const worker = new Worker({ opfs: null });
    const opened = await worker.open();
    assert.equal(opened.backend, "memory");
    assert.equal(opened.readOnly, true);
  });

  it("saves nothing while another tab holds the lock", async () => {
    const opfs = new FakeDirectory();
    const worker = new Worker({ opfs, lockTaken: true });
    const opened = await worker.open();
    assert.equal(opened.readOnly, true);
    assert.equal(opened.locked, false);
    worker.send({ type: "apply", ops: changes });
    await worker.request({ type: "read", path: "/incari/game/options.txt" });
    assert.equal(opfs.children.size, 0);
  });

  for (const kind of ["opfs", "indexeddb"]) {
    it(`downloads the client jar once and then reads it from the cache (${kind})`, async () => {
      const opfs = kind === "opfs" ? new FakeDirectory() : null;
      const indexedDB = new IDBFactory();
      let downloads = 0;
      const fetch = (url: string) => {
        downloads++;
        assert.equal(url, "http://localhost/client.jar");
        return Promise.resolve(new Response("PK jar bytes"));
      };
      const first = new Worker({ opfs, indexedDB, fetch });
      await first.open();
      const fetched = await first.request({ type: "jar", url: "http://localhost/client.jar", key: "client-26.3.jar" });
      assert.equal(fetched.cached, false);
      assert.equal(new TextDecoder().decode(fetched.data as ArrayBuffer), "PK jar bytes");

      const second = new Worker({ opfs, indexedDB, fetch });
      await second.open();
      const cached = await second.request({ type: "jar", url: "http://localhost/client.jar", key: "client-26.3.jar" });
      assert.equal(cached.cached, true);
      assert.equal(new TextDecoder().decode(cached.data as ArrayBuffer), "PK jar bytes");
      assert.equal(downloads, 1);
      // The cache isn't part of the file system the page loads.
      assert.deepEqual((await new Worker({ opfs, indexedDB, fetch }).open()).entries, []);

      // A new version's jar replaces the old one; a damaged copy can be dropped.
      await second.request({ type: "jar", url: "http://localhost/client.jar", key: "client-26.4.jar" });
      await second.request({ type: "forget", key: "client-26.4.jar" });
      const again = await second.request({ type: "jar", url: "http://localhost/client.jar", key: "client-26.3.jar" });
      assert.equal(again.cached, false);
      assert.equal(downloads, 3);
    });

    it(`keeps the client jar and the asset pack side by side (${kind})`, async () => {
      const opfs = kind === "opfs" ? new FakeDirectory() : null;
      const indexedDB = new IDBFactory();
      let downloads = 0;
      const fetch = () => {
        downloads++;
        return Promise.resolve(new Response("PK bytes"));
      };
      const first = new Worker({ opfs, indexedDB, fetch });
      await first.open();
      await first.request({ type: "jar", url: "http://localhost/client.jar", key: "client-26.3.jar" });
      await first.request({ type: "jar", url: "http://localhost/incari.pack?v=1", key: "pack-1" });
      // A new build's pack replaces the old pack, and only it.
      await first.request({ type: "jar", url: "http://localhost/incari.pack?v=2", key: "pack-2" });

      const second = new Worker({ opfs, indexedDB, fetch });
      await second.open();
      assert.equal(
        (await second.request({ type: "jar", url: "http://localhost/client.jar", key: "client-26.3.jar" })).cached,
        true,
      );
      assert.equal(
        (await second.request({ type: "jar", url: "http://localhost/incari.pack?v=2", key: "pack-2" })).cached,
        true,
      );
      assert.equal(
        (await second.request({ type: "jar", url: "http://localhost/incari.pack?v=1", key: "pack-1" })).cached,
        false,
      );
      assert.equal(downloads, 4);
    });
  }

  it("reports a failed download", async () => {
    const worker = new Worker({
      opfs: new FakeDirectory(),
      fetch: () => Promise.resolve(new Response("gone", { status: 404 })),
    });
    await worker.open();
    const reply = await worker.request({ type: "jar", url: "http://localhost/client.jar", key: "k" });
    assert.match(String(reply.error), /HTTP 404/);
  });
});
