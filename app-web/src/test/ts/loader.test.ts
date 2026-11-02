import assert from "node:assert/strict";
import { describe, it } from "node:test";

import type { BrowserFeatures } from "../../main/ts/features.ts";
import {
  buildFor,
  buildName,
  buildOptions,
  countingResponse,
  DEFAULT_MANIFEST,
  formatBytes,
  initialBuild,
  missingWasmGc,
  missingWebgl2,
  parseManifest,
  requestedBuild,
  rustPage,
  sizeOf,
  versioned,
  type BuildManifest,
} from "../../main/ts/loader.ts";

const everything: BrowserFeatures = {
  wasmGc: true,
  jspi: true,
  crossOriginIsolated: true,
  webgl2: true,
  webgpu: true,
};
/** Safari 18.2+ and Chrome 119-136: WebAssembly GC, no JSPI. */
const noJspi: BrowserFeatures = { ...everything, jspi: false };

const manifest: BuildManifest = {
  format: 2,
  id: "abc",
  game: "26.3",
  release: true,
  fast: { runtime: "incari.wasm-runtime.js", module: "incari-fast.wasm" },
  compatible: { runtime: "incari.wasm-runtime.js", module: "incari.wasm" },
  files: [
    { path: "incari.wasm", size: 1000, sha256: "0123456789abcdef0123456789abcdef" },
    { path: "incari-fast.wasm", size: 2000, sha256: "fedcba9876543210fedcba9876543210" },
  ],
};

describe("buildOptions", () => {
  it("offers both builds, Fast first, where both can run", () => {
    assert.deepEqual(buildOptions(everything, manifest), [
      { kind: "fast", available: true },
      { kind: "compatible", available: true },
    ]);
  });

  it("explains why Fast can't run in a browser without JSPI", () => {
    assert.deepEqual(buildOptions(noJspi, manifest), [
      {
        kind: "fast",
        available: false,
        reason: "Fast needs Chrome or Edge 137, Firefox 153 or newer.",
      },
      { kind: "compatible", available: true },
    ]);
  });

  it("offers Rust last, only when the copy has it, and says when WebGPU is missing", () => {
    const withRust: BuildManifest = { ...manifest, rust: { page: "rust/index.html" } };
    assert.deepEqual(buildOptions(everything, withRust)[2], { kind: "rust", available: true });
    const noWebgpu = buildOptions({ ...everything, webgpu: false }, withRust).find((o) => o.kind === "rust");
    assert.equal(noWebgpu?.available, false);
    assert.match(noWebgpu.reason ?? "", /WebGPU/);
  });

  it("marks a build this copy doesn't include", () => {
    const fastOnly = { ...manifest, compatible: undefined } as unknown as BuildManifest;
    assert.deepEqual(buildOptions(everything, fastOnly), [
      { kind: "fast", available: true },
      { kind: "compatible", available: false, reason: "This copy of the game doesn't include Compatible." },
    ]);
  });
});

describe("requestedBuild", () => {
  it("reads ?build=fast and ?build=compatible, and nothing else", () => {
    assert.equal(requestedBuild("fast"), "fast");
    assert.equal(requestedBuild("compatible"), "compatible");
    assert.equal(requestedBuild("rust"), "rust");
    assert.equal(requestedBuild("wasm"), null);
    assert.equal(requestedBuild(null), null);
  });
});

describe("initialBuild", () => {
  const both = buildOptions(everything, manifest);

  it("starts on Fast, the recommended build", () => {
    assert.equal(initialBuild(both, null), "fast");
  });

  it("never starts on Rust unless it was the last pick or nothing else runs", () => {
    const withRust = buildOptions(everything, { ...manifest, rust: { page: "rust/index.html" } });
    assert.equal(initialBuild(withRust, null), "fast");
    assert.equal(initialBuild(withRust, "rust"), "rust");
    const rustOnly = withRust.map((option) => (option.kind === "rust" ? option : { ...option, available: false }));
    assert.equal(initialBuild(rustOnly, null), "rust");
  });

  it("starts on the player's last pick while it can still run", () => {
    assert.equal(initialBuild(both, "compatible"), "compatible");
    assert.equal(initialBuild(buildOptions(noJspi, manifest), "fast"), "compatible");
  });
});

describe("buildFor", () => {
  it("gives the manifest's entry for a build that can run", () => {
    assert.deepEqual(buildFor("fast", buildOptions(everything, manifest), manifest), {
      kind: "fast",
      target: manifest.fast,
    });
  });

  it("gives the Rust build's page", () => {
    const withRust: BuildManifest = { ...manifest, rust: { page: "rust/index.html" } };
    assert.deepEqual(buildFor("rust", buildOptions(everything, withRust), withRust), {
      kind: "rust",
      target: { page: "rust/index.html" },
    });
  });

  it("refuses a build that can't run here, with the reason", () => {
    const choice = buildFor("fast", buildOptions(noJspi, manifest), manifest);
    assert.ok(choice.kind === "unsupported");
    assert.match(choice.reason, /Fast needs/);
  });
});

describe("buildName", () => {
  it("names the builds as the player sees them", () => {
    assert.equal(buildName("fast"), "Fast");
    assert.equal(buildName("compatible"), "Compatible");
    assert.equal(buildName("rust"), "Rust");
  });
});

describe("rustPage", () => {
  it("opens Rust's page beside this one, keeping the address options but ?build", () => {
    assert.equal(
      rustPage({ page: "rust/index.html" }, "https://example.test/incari/?build=rust&args=--world%20T"),
      "https://example.test/incari/rust/index.html?args=--world+T",
    );
    assert.equal(
      rustPage({ page: "rust/index.html" }, "https://example.test/"),
      "https://example.test/rust/index.html",
    );
  });
});

describe("missingWebgl2 and missingWasmGc", () => {
  it("need WebGL 2 and WebAssembly GC for either build", () => {
    assert.equal(missingWebgl2(everything), null);
    assert.match(missingWebgl2({ ...everything, webgl2: false }) ?? "", /WebGL 2/);
    assert.equal(missingWasmGc(everything), null);
    assert.match(missingWasmGc({ ...everything, wasmGc: false }) ?? "", /too old to run the game/);
  });
});

describe("parseManifest", () => {
  it("accepts a manifest the build tool writes", () => {
    assert.deepEqual(parseManifest(JSON.parse(JSON.stringify(manifest))), manifest);
  });

  it("refuses another format, a missing file list, and a manifest with nothing to run", () => {
    assert.throws(() => parseManifest({ ...manifest, format: 1 }), /format 1/);
    assert.throws(() => parseManifest({ format: 2, id: "x" }), /file list/);
    assert.throws(() => parseManifest({ format: 2, id: "x", files: [] }), /no build/);
    assert.throws(() => parseManifest(null), /isn't an object/);
  });
});

describe("versioned", () => {
  it("adds the file's hash as its version", () => {
    assert.equal(versioned(manifest, "incari.wasm"), "incari.wasm?v=0123456789abcdef");
  });

  it("leaves a file the manifest doesn't list as it is", () => {
    assert.equal(versioned(DEFAULT_MANIFEST, "incari.wasm"), "incari.wasm");
    assert.equal(sizeOf(DEFAULT_MANIFEST, "incari.wasm"), 0);
    assert.equal(sizeOf(manifest, "incari-fast.wasm"), 2000);
  });
});

describe("countingResponse", () => {
  it("reports the bytes read so far and keeps the body and headers", async () => {
    const original = new Response(new Blob([new Uint8Array(10), new Uint8Array(5)]), {
      headers: { "Content-Type": "application/wasm" },
    });
    const seen: number[] = [];
    const counted = countingResponse(original, (received) => seen.push(received));
    assert.equal(counted.headers.get("Content-Type"), "application/wasm");
    const body = new Uint8Array(await counted.arrayBuffer());
    assert.equal(body.byteLength, 15);
    assert.equal(seen.at(-1), 15);
  });
});

describe("formatBytes", () => {
  it("prints sizes as people read them", () => {
    assert.equal(formatBytes(512), "512 B");
    assert.equal(formatBytes(1_500), "1.5 kB");
    assert.equal(formatBytes(29_390_000), "29 MB");
    assert.equal(formatBytes(156_500_000), "157 MB");
  });
});
