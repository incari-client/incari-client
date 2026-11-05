# ADR 0011: Storage: the file system in memory, written behind to OPFS by a worker

- Status: accepted
- Date: 2026-09-28

## Context

The browser build ran on TeaVM's in-memory file system: a reload lost everything, and the
client jar was unpacked on every load (22,844 resources, 86 MB of heap, ~1 s).

The game's file access is synchronous everywhere; the browser's durable storage (OPFS, IndexedDB)
isn't, and OPFS's synchronous handles exist only in workers. Making file calls wait would turn most
of the program into coroutines (ADR 0006), and the page's thread may not block on a worker.

## Decision

- **The file system is a tree in memory** (`TreeFileSystem`), installed first thing in `WebMain`.
  Every file call stays synchronous. Where TeaVM differed from the JDK, it follows the JDK
  (`"rw"` keeps bytes, seeks past the end leave zeros, renames may replace, `/tmp` exists).
- **Written behind once a frame:** a `Journal` records shape changes and written byte ranges. At
  `endFrame` one batch goes out: folders, then writes (new files whole, existing ones only the
  changed bytes), then deletes. A file rewritten many times is sent once; a temp file renamed into
  place is never sent under its temp name. No game patches: save points are ordinary writes.
- **A storage worker keeps the durable copy** (`storage-worker.js`, run from a Blob URL). OPFS with
  sync access handles, writing only changed bytes off the page's thread; `createWritable` where sync
  handles are missing; IndexedDB without OPFS (plain-http pages, older browsers); nothing, with a
  console note, if neither exists. It loads `/incari` at start-up and hands it to the page.
- **One tab writes:** a Web Lock; a second tab keeps its changes to itself and tells the player
  (`PlatformNotice`, a toast), as it does when there's no storage. The page asks for persistent storage.
- **A change the store couldn't keep is sent again whole** (2026-11): the worker stores the rest of
  the batch and names the failed paths; five seconds later the page marks them changed, so a lost
  range never leaves a file damaged. A failed save is told to the player once until saving works again.
- **Leaving saves singleplayer** (2026-11): the page's own hidden, `beforeunload` and `pagehide`
  listeners post a save to the world's worker, and in a world the page asks before closing.
- **The client jar is cached whole and mounted** read-only at `/incari/classpath` (`ZipArchive`),
  each entry inflated when opened: one read instead of 22,844, and a 40 MB buffer outside the heap
  instead of 86 MB in it.
- **Not kept:** dropped files, the mounted jar, anything outside `/incari`.
- **Files reach the player through the shell:** `PlatformShell.openPath` downloads a file (F2's
  screenshot link) or opens the file picker for a folder (the pack screens' "Open Pack Folder" becomes import).
- **Screenshots encode in the page** (`PngEncoder` behind LWJGL's `STBImageWrite`).
- **Fixed on the way:** `MessageDigest` added (Guava's `sha1()` names server icons); TeaVM's
  `ZipFile` failed at some entries' end (638 textures of a pack); single-byte reads returned signed
  bytes, so 0x80 read as end-of-file and the server list was lost.

## Alternatives rejected

- **Suspending file calls:** most of the game becomes coroutines, and each small write a round trip.
- **The page blocking on a worker** (`Atomics.wait`): the page's thread may not block.
- **Writing from the page with `createWritable`:** copies the whole file on every close.
- **Persisting the unpacked jar:** 22,844 reads every start.
- **Flushing only on `pagehide`:** no Java can run there, and async writes started there may not finish.

## Consequences

- **A reload can lose** the current frame's writes and a batch not yet applied (milliseconds). An
  interrupted OPFS batch can be partly applied; IndexedDB batches are whole. A failed change is
  sent again, whole.
- **Everything stored is in memory** (worlds too), until region files stay in place (ADR 0041).
- **File times:** OPFS keeps its own; a time the game sets is kept only in IndexedDB. Read-only flags aren't kept.
- **Without persistent storage**, the browser may evict data when the disk is nearly full.
- The game's profiling-report zip file system is still unsupported.
- **Verified** by `storage.steps`, `community-pack.steps`, `screenshot.steps`, the probes
  `StorageProbe`, `FileReadProbe`, `ZipFileProbe`, and unit tests of the tree, journal, zip reader and
  worker. Persistence across a browser restart, second tabs and downloads are on the manual checklist.
