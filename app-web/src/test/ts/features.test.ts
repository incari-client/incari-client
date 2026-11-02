import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { detectFeatures, type FeatureScope } from "../../main/ts/features.ts";

/** JSPI's two members, which TypeScript's WebAssembly types don't have yet. */
function jspiOf(api: object): { Suspending: unknown; promising: unknown } {
  const jspi = api as { Suspending: unknown; promising: unknown };
  return { Suspending: jspi.Suspending, promising: jspi.promising };
}

describe("detectFeatures", () => {
  it("finds WebAssembly GC in an engine that has it", async () => {
    // Node 24+ ships the GC proposal, so the real probe must validate here.
    assert.equal((await detectFeatures({ WebAssembly })).wasmGc, true);
  });

  it("finds JSPI in an engine that has it, and none without WebAssembly GC", async () => {
    // Node 26 ships JSPI, as Chrome 137+ does.
    assert.equal((await detectFeatures({ WebAssembly })).jspi, true);
    assert.equal((await detectFeatures({ WebAssembly: { validate: () => true } })).jspi, false);
    assert.equal(
      (
        await detectFeatures({
          WebAssembly: { ...jspiOf(WebAssembly), validate: () => false } as unknown as typeof WebAssembly,
        })
      ).jspi,
      false,
    );
  });

  it("reports no WebAssembly GC when the engine rejects the probe", async () => {
    const withoutGc: FeatureScope = { WebAssembly: { validate: () => false } };
    assert.equal((await detectFeatures(withoutGc)).wasmGc, false);
  });

  it("treats an engine that throws on the probe as unsupported", async () => {
    const throwing: FeatureScope = {
      WebAssembly: {
        validate: () => {
          throw new TypeError("unknown type form");
        },
      },
    };
    assert.equal((await detectFeatures(throwing)).wasmGc, false);
  });

  it("needs both isolation and SharedArrayBuffer for shared memory", async () => {
    assert.equal((await detectFeatures({ crossOriginIsolated: true, SharedArrayBuffer })).crossOriginIsolated, true);
    assert.equal((await detectFeatures({ crossOriginIsolated: false, SharedArrayBuffer })).crossOriginIsolated, false);
    assert.equal((await detectFeatures({ crossOriginIsolated: true })).crossOriginIsolated, false);
  });

  it("reports no WebGL2 without a document, or when no context can be made", async () => {
    assert.equal((await detectFeatures({})).webgl2, false);
    const noContext = {
      createElement: () => ({ getContext: () => null }),
    } as unknown as Pick<Document, "createElement">;
    assert.equal((await detectFeatures({ document: noContext })).webgl2, false);
  });

  it("finds WebGPU only when it gives an adapter", async () => {
    assert.equal((await detectFeatures({})).webgpu, false);
    const gpu = (adapter: unknown) => ({ navigator: { gpu: { requestAdapter: () => Promise.resolve(adapter) } } });
    assert.equal((await detectFeatures(gpu({}))).webgpu, true);
    // A browser with the API and no GPU it will use.
    assert.equal((await detectFeatures(gpu(null))).webgpu, false);
    const failing = { navigator: { gpu: { requestAdapter: () => Promise.reject(new Error("no GPU process")) } } };
    assert.equal((await detectFeatures(failing)).webgpu, false);
  });
});
