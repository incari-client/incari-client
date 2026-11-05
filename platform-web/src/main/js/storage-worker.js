// @ts-check
// Incari's storage worker (ADR 0011). The page keeps its file system in memory; this worker
// keeps the durable copy: in the origin private file system (OPFS) where the browser has it,
// written with synchronous access handles (which only workers get), or in IndexedDB where it
// doesn't (insecure origins, older browsers). It also caches the client jar.
//
// Runs as a classic worker created from a Blob URL (platform-web's BrowserStore), so the page needs
// no extra file. Messages, each answered with the same `id` except "apply" (fire and forget):
//   {type: "open", id, root, lock, inPlace?} -> {id, backend, readOnly, entries, error?}
//   {type: "apply", ops}            -> {type: "applied", count, error?, failed}
//                                      (failed: the paths not stored, which the page sends again
//                                      whole; the batch's other changes are stored all the same)
//   {type: "jar", id, url, key}     -> {id, data?, cached, error?}
//   {type: "forget", id, key}       -> {id}
//   {type: "read", id, path}        -> {id, data: ArrayBuffer | null}
//   {type: "list", id, path}        -> {id, paths: string[]} (everything stored under a folder)
//   {type: "stat", id, path}        -> {id, entries} (as "open"'s, with sizes and no bytes)
//   {type: "sync", id}              -> {id} (once everything sent before it is stored)
//   {type: "port", port}            -> nothing; the port's messages are handled like the page's,
//                                      in the same queue, and answered on it (ADR 0041)
// Messages are handled one at a time, in order, whichever port they come from.
//
// In-place files (ADR 0041): with OPFS, "open" lists the files `inPlace` describes (a world's
// region files) with their sizes and without their bytes ({stored: true}). The page reads one only
// when it opens it, and singleplayer's server in a worker reads and writes them where they are.

"use strict";

/**
 * A change to the store: a folder, a file's size and some of its bytes, or nothing at a path.
 * @typedef {{op: "mkdir", path: string, mtime: number}
 *   | {op: "write", path: string, size: number, at: number, data: Uint8Array<ArrayBuffer> | null, mtime: number}
 *   | {op: "delete", path: string}} Op
 */

/**
 * A file or folder of the store, as the page loads it. A stored entry is an in-place file: its
 * size, and no bytes.
 * @typedef {{path: string, dir: boolean, mtime: number, data: ArrayBuffer | null, size?: number, stored?: boolean}} Entry
 */

/**
 * Which files stay where they are, never loaded whole: those inside a folder named `within`
 * (anywhere in the path) whose names end with one of `suffixes`.
 * @typedef {{within: string, suffixes: string[]}} InPlace
 */

/**
 * @param {InPlace | null} inPlace
 * @param {string} path
 */
function isInPlace(inPlace, path) {
  return inPlace !== null && path.includes(inPlace.within) && inPlace.suffixes.some((suffix) => path.endsWith(suffix));
}

/**
 * @typedef {object} Backend
 * @property {string} kind "opfs" or "indexeddb" (or "memory": nothing survives)
 * @property {(root: string, inPlace: InPlace | null) => Promise<Entry[]>} load everything under a folder
 * @property {(root: string) => Promise<Entry[]>} stat everything under a folder: sizes, no bytes
 * @property {(ops: Op[]) => Promise<Applied>} apply
 * @property {(key: string) => Promise<ArrayBuffer | null>} cached
 * @property {(key: string, data: ArrayBuffer) => Promise<void>} cache replaces the whole cache
 * @property {(key: string) => Promise<void>} forget
 * @property {(path: string) => Promise<ArrayBuffer | null>} read one file
 */

/**
 * What applying a batch did: the paths whose change wasn't stored, and why the first one wasn't.
 * @typedef {{failed: string[], error: string | null}} Applied
 */

/**
 * The paths a batch changes, each once: all of them failed when a batch is stored whole or not at all.
 * @param {Op[]} ops
 * @returns {string[]}
 */
function pathsOf(ops) {
  return [...new Set(ops.map((op) => op.path))];
}

/**
 * The parts of a synchronous access handle this worker uses (lib.dom doesn't declare it).
 * @typedef {{
 *   truncate(size: number): void,
 *   write(data: ArrayBufferView, options: {at: number}): number,
 *   flush(): void,
 *   close(): void}} SyncHandle
 */

/**
 * The parts of an OPFS directory handle this worker uses.
 * @typedef {{
 *   getDirectoryHandle(name: string, options?: {create?: boolean}): Promise<DirectoryHandle>,
 *   getFileHandle(name: string, options?: {create?: boolean}): Promise<FileHandle>,
 *   removeEntry(name: string, options?: {recursive?: boolean}): Promise<void>,
 *   entries(): AsyncIterable<[string, DirectoryHandle | FileHandle]>,
 *   kind: "directory"}} DirectoryHandle
 */

/**
 * @typedef {{
 *   getFile(): Promise<File>,
 *   createSyncAccessHandle?: () => Promise<SyncHandle>,
 *   createWritable?: (options?: {keepExistingData?: boolean}) => Promise<FileSystemWritableFileStream>,
 *   kind: "file"}} FileHandle
 */

/** Where the client jar is cached in OPFS: beside the file system's folder, never loaded with it. */
const CACHE_FOLDER = ".incari-cache";

/**
 * Whether `error` is a DOMException (or error) of this name. By shape, not by class: the
 * browser's DOMExceptions and a test's stand-ins come from different places.
 * @param {unknown} error
 * @param {string} name
 */
function isError(error, name) {
  return typeof error === "object" && error !== null && "name" in error && error.name === name;
}

/** @param {unknown} error */
function describe(error) {
  if (typeof error === "object" && error !== null && "name" in error && "message" in error) {
    return `${String(error.name)}: ${String(error.message)}`;
  }
  return String(error);
}

/** @param {string} path */
function parts(path) {
  return path.split("/").filter((part) => part.length > 0);
}

/**
 * The store in the origin private file system: a folder per folder, a file per file, under the
 * same paths as the page's (so "/incari/game/options.txt" is incari/game/options.txt there).
 * @param {DirectoryHandle} root
 * @returns {Backend}
 */
function opfsBackend(root) {
  /**
   * @param {string[]} names
   * @param {boolean} create
   * @returns {Promise<DirectoryHandle | null>}
   */
  async function folder(names, create) {
    let directory = root;
    for (const name of names) {
      try {
        directory = await directory.getDirectoryHandle(name, { create });
      } catch (error) {
        if (create && isError(error, "TypeMismatchError")) {
          // A file where a folder now is: the page's tree replaced it.
          await directory.removeEntry(name);
          directory = await directory.getDirectoryHandle(name, { create: true });
        } else if (!create && (isError(error, "NotFoundError") || isError(error, "TypeMismatchError"))) {
          return null;
        } else {
          throw error;
        }
      }
    }
    return directory;
  }

  /**
   * @param {DirectoryHandle} directory
   * @param {string} name
   * @returns {Promise<FileHandle>}
   */
  async function fileIn(directory, name) {
    try {
      return await directory.getFileHandle(name, { create: true });
    } catch (error) {
      if (!isError(error, "TypeMismatchError")) {
        throw error;
      }
      await directory.removeEntry(name, { recursive: true });
      return await directory.getFileHandle(name, { create: true });
    }
  }

  /**
   * Sets a file's size and writes bytes into it, a range at a time (a file's changes since the
   * page's last flush can be several). A synchronous access handle writes in place; where there's
   * none, a writable stream copies the file and swaps it in on close.
   * @param {FileHandle} handle
   * @param {number} size
   * @param {{at: number, data: Uint8Array<ArrayBuffer> | null}[]} writes
   */
  async function writeInto(handle, size, writes) {
    if (handle.createSyncAccessHandle) {
      const access = await handle.createSyncAccessHandle();
      try {
        access.truncate(size);
        for (const { at, data } of writes) {
          if (data && data.byteLength > 0) {
            access.write(data, { at });
          }
        }
        access.flush();
      } finally {
        access.close();
      }
    } else if (handle.createWritable) {
      const stream = await handle.createWritable({ keepExistingData: true });
      await stream.truncate(size);
      for (const { at, data } of writes) {
        if (data && data.byteLength > 0) {
          await stream.write({ type: "write", position: at, data });
        }
      }
      await stream.close();
    } else {
      throw new Error("This browser can't write to the origin private file system");
    }
  }

  /**
   * @param {DirectoryHandle} directory
   * @param {string} path
   * @param {Entry[]} out
   * @param {(path: string) => boolean} storedOnly which files to list without their bytes
   */
  async function walk(directory, path, out, storedOnly) {
    for await (const [name, handle] of directory.entries()) {
      const child = `${path}/${name}`;
      if (handle.kind === "directory") {
        out.push({ path: child, dir: true, mtime: 0, data: null });
        await walk(handle, child, out, storedOnly);
      } else {
        const file = await handle.getFile();
        if (storedOnly(child)) {
          out.push({ path: child, dir: false, mtime: file.lastModified, data: null, size: file.size, stored: true });
        } else {
          out.push({ path: child, dir: false, mtime: file.lastModified, data: await file.arrayBuffer() });
        }
      }
    }
  }

  /**
   * @param {string} rootPath
   * @param {(path: string) => boolean} storedOnly
   */
  async function loadUnder(rootPath, storedOnly) {
    const directory = await folder(parts(rootPath), false);
    /** @type {Entry[]} */
    const entries = [];
    if (directory) {
      await walk(directory, rootPath === "/" ? "" : rootPath, entries, storedOnly);
    }
    return entries;
  }

  /** @param {string} path */
  async function readFile(path) {
    const names = parts(path);
    const name = names.pop();
    const directory = await folder(names, false);
    if (!directory || name === undefined) {
      return null;
    }
    try {
      const handle = await directory.getFileHandle(name);
      return await (await handle.getFile()).arrayBuffer();
    } catch (error) {
      if (isError(error, "NotFoundError") || isError(error, "TypeMismatchError")) {
        return null;
      }
      throw error;
    }
  }

  return {
    kind: "opfs",
    load: (rootPath, inPlace) => loadUnder(rootPath, (path) => isInPlace(inPlace, path)),
    stat: (rootPath) => loadUnder(rootPath, () => true),
    async apply(ops) {
      // Each change on its own: one that fails (a full disk, a file held open elsewhere) mustn't
      // keep the rest of the batch from being stored. The page sends a failed path again, whole:
      // its writes are only the bytes that changed, and a range lost here would leave the file
      // damaged for good.
      /** @type {Applied} */
      const result = { failed: [], error: null };
      for (let i = 0; i < ops.length; i++) {
        const op = /** @type {Op} */ (ops[i]);
        const names = parts(op.path);
        try {
          if (op.op === "mkdir") {
            await folder(names, true);
          } else if (op.op === "write") {
            // The file's next writes in this batch go through the same handle.
            const writes = [{ at: op.at, data: op.data }];
            let size = op.size;
            for (let next = ops[i + 1]; next?.op === "write" && next.path === op.path; next = ops[i + 1]) {
              writes.push({ at: next.at, data: next.data });
              size = next.size;
              i++;
            }
            const name = names.pop();
            const directory = await folder(names, true);
            if (directory && name !== undefined) {
              await writeInto(await fileIn(directory, name), size, writes);
            }
          } else {
            const name = names.pop();
            const directory = await folder(names, false);
            if (directory && name !== undefined) {
              try {
                await directory.removeEntry(name, { recursive: true });
              } catch (error) {
                if (!isError(error, "NotFoundError")) {
                  throw error;
                }
              }
            }
          }
        } catch (error) {
          if (!result.failed.includes(op.path)) {
            result.failed.push(op.path);
          }
          result.error ??= describe(error);
        }
      }
      return result;
    },
    async cached(key) {
      return readFile(`/${CACHE_FOLDER}/${key}`);
    },
    async cache(key, data) {
      const directory = await folder([CACHE_FOLDER], true);
      if (!directory) {
        return;
      }
      // One version of each archive at a time: a new version's replaces the old one's.
      /** @type {string[]} */
      const old = [];
      for await (const [name] of directory.entries()) {
        if (name !== key && sameArchive(name, key)) {
          old.push(name);
        }
      }
      for (const name of old) {
        await directory.removeEntry(name, { recursive: true });
      }
      const bytes = new Uint8Array(data);
      await writeInto(await fileIn(directory, key), bytes.byteLength, [{ at: 0, data: bytes }]);
    },
    async forget(key) {
      const directory = await folder([CACHE_FOLDER], false);
      if (directory) {
        try {
          await directory.removeEntry(key);
        } catch (error) {
          if (!isError(error, "NotFoundError")) {
            throw error;
          }
        }
      }
    },
    read: readFile,
  };
}

/**
 * Whether two cache keys are versions of the same archive: the part before the first "-" names it
 * ("client-26.3.jar", "pack-<hash>"), so caching the asset pack doesn't evict the client jar.
 * @param {string} a
 * @param {string} b
 * @returns {boolean}
 */
function sameArchive(a, b) {
  return a.split("-", 1)[0] === b.split("-", 1)[0];
}

/**
 * @template T
 * @param {IDBRequest<T>} request
 * @returns {Promise<T>}
 */
function done(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => {
      resolve(request.result);
    };
    request.onerror = () => {
      reject(request.error ?? new Error("IndexedDB request failed"));
    };
  });
}

/**
 * A stored value, of whatever type it was stored as.
 * @param {IDBRequest} request
 * @returns {Promise<unknown>}
 */
function value(request) {
  return done(request);
}

/** @param {IDBTransaction} transaction */
function committed(transaction) {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => {
      resolve(undefined);
    };
    transaction.onerror = () => {
      reject(transaction.error ?? new Error("IndexedDB transaction failed"));
    };
    transaction.onabort = () => {
      reject(transaction.error ?? new Error("IndexedDB transaction aborted"));
    };
  });
}

/**
 * The store in IndexedDB: a record per file or folder, keyed by path. Each batch of changes is
 * one transaction, so it lands whole or not at all.
 * @param {IDBFactory} factory
 * @param {typeof IDBKeyRange} keyRange
 * @returns {Promise<Backend>}
 */
async function indexedDbBackend(factory, keyRange) {
  const open = factory.open("incari-storage", 1);
  open.onupgradeneeded = () => {
    open.result.createObjectStore("files", { keyPath: "path" });
    open.result.createObjectStore("cache");
  };
  /** @type {IDBDatabase} */
  const db = await done(open);

  /** @param {string} path */
  function below(path) {
    return keyRange.bound(`${path}/`, `${path}/\uffff`);
  }

  /**
   * @param {string} rootPath
   * @returns {Promise<Entry[]>}
   */
  async function loadAll(rootPath) {
    const transaction = db.transaction("files", "readonly");
    /** @type {Entry[]} */
    const records = await done(transaction.objectStore("files").getAll(below(rootPath === "/" ? "" : rootPath)));
    return records.map((record) => ({
      path: record.path,
      dir: record.dir,
      mtime: record.mtime,
      data: record.data,
    }));
  }

  return {
    kind: "indexeddb",
    // Every file loaded whole: IndexedDB stores whole values, so nothing stays in place.
    load: (rootPath) => loadAll(rootPath),
    async stat(rootPath) {
      return (await loadAll(rootPath)).map((entry) => ({
        path: entry.path,
        dir: entry.dir,
        mtime: entry.mtime,
        data: null,
        size: entry.data ? entry.data.byteLength : 0,
        stored: !entry.dir,
      }));
    },
    async apply(ops) {
      const transaction = db.transaction("files", "readwrite");
      const finished = committed(transaction);
      const files = transaction.objectStore("files");
      for (const op of ops) {
        if (op.op === "mkdir") {
          files.put({ path: op.path, dir: true, mtime: op.mtime, data: null });
        } else if (op.op === "write") {
          let data;
          if (op.at === 0 && (op.data?.byteLength ?? 0) === op.size) {
            data = op.data ? op.data.slice().buffer : new ArrayBuffer(0);
          } else {
            // Part of a file: patch the stored copy (IndexedDB only stores whole values).
            const stored = /** @type {Entry | undefined} */ (await value(files.get(op.path)));
            const bytes = new Uint8Array(op.size);
            if (stored?.data) {
              bytes.set(new Uint8Array(stored.data, 0, Math.min(stored.data.byteLength, op.size)));
            }
            if (op.data) {
              bytes.set(op.data.subarray(0, Math.max(0, op.size - op.at)), op.at);
            }
            data = bytes.buffer;
          }
          files.put({ path: op.path, dir: false, mtime: op.mtime, data });
        } else {
          files.delete(op.path);
          files.delete(below(op.path));
        }
      }
      try {
        await finished;
      } catch (error) {
        // One transaction: none of it was stored.
        return { failed: pathsOf(ops), error: describe(error) };
      }
      return { failed: [], error: null };
    },
    async cached(key) {
      const data = /** @type {ArrayBuffer | undefined} */ (
        await value(db.transaction("cache", "readonly").objectStore("cache").get(key))
      );
      return data ?? null;
    },
    async cache(key, data) {
      const transaction = db.transaction("cache", "readwrite");
      const finished = committed(transaction);
      const store = transaction.objectStore("cache");
      const keys = await done(store.getAllKeys());
      for (const name of keys) {
        if (typeof name === "string" && name !== key && sameArchive(name, key)) {
          store.delete(name);
        }
      }
      store.put(data, key);
      await finished;
    },
    async forget(key) {
      const transaction = db.transaction("cache", "readwrite");
      const finished = committed(transaction);
      transaction.objectStore("cache").delete(key);
      await finished;
    },
    async read(path) {
      const record = /** @type {Entry | undefined} */ (
        await value(db.transaction("files", "readonly").objectStore("files").get(path))
      );
      return record && !record.dir ? record.data : null;
    },
  };
}

/** @returns {Backend} */
function memoryBackend() {
  return {
    kind: "memory",
    load: () => Promise.resolve([]),
    stat: () => Promise.resolve([]),
    apply: () => Promise.resolve({ failed: [], error: null }),
    cached: () => Promise.resolve(null),
    cache: () => Promise.resolve(),
    forget: () => Promise.resolve(),
    read: () => Promise.resolve(null),
  };
}

/**
 * The best store this browser offers: OPFS, else IndexedDB, else none.
 * @param {Scope} scope
 * @param {string[]} problems where to say why a better one wasn't used
 * @returns {Promise<Backend>}
 */
async function chooseBackend(scope, problems) {
  try {
    const root = /** @type {DirectoryHandle} */ (/** @type {unknown} */ (await scope.navigator.storage.getDirectory()));
    return opfsBackend(root);
  } catch (error) {
    problems.push(`no origin private file system (${describe(error)})`);
  }
  if (scope.indexedDB && scope.IDBKeyRange) {
    try {
      return await indexedDbBackend(scope.indexedDB, scope.IDBKeyRange);
    } catch (error) {
      problems.push(`no IndexedDB (${describe(error)})`);
    }
  }
  return memoryBackend();
}

/**
 * Takes the storage lock if no other tab holds it, and keeps it for the worker's life: two tabs
 * writing the same files would each overwrite the other's changes.
 * @param {Navigator} navigator
 * @param {string} name
 * @returns {Promise<boolean>}
 */
function takeLock(navigator, name) {
  if (!("locks" in navigator)) {
    return Promise.resolve(true);
  }
  return new Promise((resolve) => {
    navigator.locks
      .request(name, { ifAvailable: true }, (lock) => {
        resolve(lock !== null);
        // Held until the worker ends.
        return lock ? new Promise(() => undefined) : undefined;
      })
      .catch(() => {
        resolve(true);
      });
  });
}

/**
 * Where a message came from, and where its answer goes: the page (the worker's own scope) or a
 * port another worker was given (singleplayer's server, ADR 0041).
 * @typedef {{postMessage(message: unknown, transfer: Transferable[]): void}} Sender
 */

/**
 * @typedef {{
 *   postMessage(message: unknown, transfer: Transferable[]): void,
 *   onmessage: ((event: MessageEvent<Record<string, unknown>>) => void) | null,
 *   navigator: Navigator,
 *   indexedDB?: IDBFactory,
 *   IDBKeyRange?: typeof IDBKeyRange,
 *   fetch: typeof fetch}} Scope
 */

/**
 * Handles the page's messages on `scope` (the worker's global scope; a stand-in in tests), and
 * those of the ports it's given.
 * @param {Scope} scope
 */
function serve(scope) {
  /** @type {Backend} */
  let backend = memoryBackend();
  let readOnly = true;
  /** @type {Promise<unknown>} */
  let queue = Promise.resolve();

  /**
   * @param {Record<string, unknown>} message
   * @param {Sender} from
   * @returns {Promise<void>}
   */
  async function handle(message, from) {
    const id = message.id;
    switch (message.type) {
      case "open": {
        /** @type {string[]} */
        const problems = [];
        const locked = await takeLock(scope.navigator, String(message.lock));
        backend = await chooseBackend(scope, problems);
        readOnly = !locked || backend.kind === "memory";
        const inPlace = /** @type {InPlace | undefined} */ (message.inPlace) ?? null;
        /** @type {Entry[]} */
        let entries;
        try {
          entries = await backend.load(String(message.root), inPlace);
        } catch (error) {
          problems.push(`couldn't load (${describe(error)})`);
          entries = [];
        }
        /** @type {Transferable[]} */
        const buffers = entries.flatMap((entry) => (entry.data ? [entry.data] : []));
        from.postMessage(
          { id, backend: backend.kind, readOnly, locked, entries, problems: problems.join("; ") },
          buffers,
        );
        return;
      }
      case "apply": {
        const ops = /** @type {Op[]} */ (message.ops);
        if (readOnly) {
          return;
        }
        /** @type {Applied} */
        let applied;
        try {
          applied = await backend.apply(ops);
        } catch (error) {
          applied = { failed: pathsOf(ops), error: describe(error) };
        }
        if (applied.error === null) {
          from.postMessage({ type: "applied", count: ops.length, failed: [] }, []);
        } else {
          from.postMessage({ type: "applied", count: ops.length, error: applied.error, failed: applied.failed }, []);
        }
        return;
      }
      case "jar": {
        const key = String(message.key);
        try {
          let data = await backend.cached(key);
          const cached = data !== null;
          if (!data) {
            const response = await scope.fetch(String(message.url));
            if (!response.ok) {
              throw new Error(`HTTP ${String(response.status)}`);
            }
            data = await response.arrayBuffer();
            if (!readOnly) {
              await backend.cache(key, data);
            }
          }
          from.postMessage({ id, data, cached }, [data]);
        } catch (error) {
          from.postMessage({ id, error: describe(error) }, []);
        }
        return;
      }
      case "forget": {
        if (!readOnly) {
          await backend.forget(String(message.key));
        }
        from.postMessage({ id }, []);
        return;
      }
      case "read": {
        /** @type {ArrayBuffer | null} */
        let data;
        try {
          data = await backend.read(String(message.path));
        } catch {
          data = null;
        }
        from.postMessage({ id, data }, data ? [data] : []);
        return;
      }
      case "list": {
        /** @type {string[]} */
        let paths;
        try {
          paths = (await backend.load(String(message.path), null)).map((entry) => entry.path).sort();
        } catch {
          paths = [];
        }
        from.postMessage({ id, paths }, []);
        return;
      }
      case "stat": {
        /** @type {Entry[]} */
        let entries;
        try {
          entries = await backend.stat(String(message.path));
        } catch {
          entries = [];
        }
        from.postMessage({ id, entries }, []);
        return;
      }
      case "sync": {
        // Messages are handled in order: everything sent before this is stored by now.
        from.postMessage({ id }, []);
        return;
      }
      case "port": {
        const port = /** @type {MessagePort} */ (message.port);
        port.onmessage = (/** @type {MessageEvent<Record<string, unknown>>} */ event) => {
          enqueue(event.data, port);
        };
        return;
      }
      default:
        from.postMessage({ id, error: `unknown message ${String(message.type)}` }, []);
    }
  }

  /**
   * @param {Record<string, unknown>} message
   * @param {Sender} from
   */
  function enqueue(message, from) {
    queue = queue.then(
      () => handle(message, from),
      () => handle(message, from),
    );
  }

  scope.onmessage = (event) => {
    enqueue(event.data, scope);
  };
}

serve(/** @type {Scope} */ (/** @type {unknown} */ (globalThis)));
