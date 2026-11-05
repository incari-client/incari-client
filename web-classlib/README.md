# web-classlib: the JDK parts TeaVM lacks

TeaVM's class library covers most of the JDK, but not all Minecraft and its libraries use
(`CompletableFuture`, executors, locks, `HexFormat`, `ScopedValue`…). This module fills the gaps
for the browser build only. The desktop runs on a real JDK and never loads it.

## Layout

| Where | What |
| --- | --- |
| `substitutes.<pkg>.T<Name>` | Stands in for `<pkg>.<Name>`, as TeaVM names its own (`ClasslibPolicy` maps them) |
| `additions.<pkg>.<Name>Additions` | Methods added to a class TeaVM has. `@InstanceMethod`, `@Replaces` (fixes a TeaVM method) and `@Constructor` mark how |
| `Removed` | What the build turns removed references into |
| `memory.Heap` | Off-heap memory behind LWJGL's `MemoryUtil`, `MemoryStack`, `BufferUtils`: linear memory in the browser, heap buffers on the JVM (ADR 0008) |
| `memory.PageHeap` | `Runtime`'s memory figures from Chromium's `performance.memory` (TeaVM reports `MAX_VALUE`) |
| `image` | PNG decode and encode, image resize, behind LWJGL's STB |
| `resources.ClassPathResources` | Class-path resources from the unpacked client jar |
| `security.Digest` | MD5, SHA-1 and SHA-256 behind `MessageDigest` (Guava's hashing needs them) |
| `security.RsaPublicKey`, `RsaPrivateKey` | RSA keys behind `KeyFactory`, `Cipher` (RSA/PKCS#1, AES/CFB8), `Signature` (SHA256withRSA) and `KeyGenerator`: the login handshake and chat signing, over Bouncy Castle (ADR 0070) |
| `logging`, `concurrent`, `nio`, `util` | Helpers: the console SLF4J binding, file channels, and more |

## TeaVM behaviour to design around

| TeaVM does | So |
| --- | --- |
| `FileInputStream.read()`/`RandomAccessFile.read()` return signed bytes | Replaced by additions (`FileReadProbe`) |
| `RandomAccessFile(f, "rw")` doesn't create a missing file | `FileChannel.open` with `CREATE` creates it first |
| `ThreadLocal` reads a weak map (a JavaScript call) every time | `TThreadLocal` caches the last thread's slot (11,000 → 900 calls/s) |
| `ConcurrentMap.computeIfAbsent` returns null after inserting | `TConcurrentMap` has correct defaults |
| `Files.find` fails on non-matching directories | `FilesAdditions.find` replaces it |
| `java.lang.invoke` is empty, so `MethodHandles.lookup()` fails | A lookup that finds nothing, so libraries fall back |
| `Class.forName("java.lang.Float")` fails | JOML's `Runtime` is substituted |
| Some methods are `@UnsupportedOn(WEBASSEMBLY_GC)` | An addition with the same signature replaces them |
| Timed waits are millisecond-only | Waits round up to 1 ms |
| A `fillInStackTrace` override that can suspend makes every exception constructor a coroutine | Keep overrides trivial; `async.txt` shows the chain |
| No module system | `Module`/`ModuleLayer` model one unnamed boot layer |

## Invariants

- Written fresh from the JDK's documented behaviour. Never copy JDK or TeaVM source.
- Browser threads take turns on one core: `synchronized`, `wait` and `sleep` suspend. Never busy-wait.
- Everything must also be correct on a real multi-threaded JVM, where the tests run.
- Public signatures match the JDK's once `T` names are read as JDK names.
- Under `substitutes`, a top-level class starting with `T` is a substitute. Put helpers elsewhere.
- Use only what the browser build has.

## Tests

```sh
./gradlew :web-classlib:check
```
