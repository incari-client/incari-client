import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { staleBuildEntries, strategyFor } from "../../main/ts/cache-policy.ts";

const scope = new URL("https://example.test/incari/");
const at = (path: string) => new URL(path, scope);

describe("strategyFor", () => {
  it("keeps versioned build files until their version changes", () => {
    assert.equal(strategyFor(at("incari.wasm?v=0123456789abcdef"), scope), "build");
    assert.equal(strategyFor(at("incari.pack?v=0123456789abcdef"), scope), "build");
  });

  it("keeps game assets named by their content hash", () => {
    assert.equal(strategyFor(at("assets/objects/ab/ab" + "0".repeat(38)), scope), "immutable");
    assert.equal(strategyFor(at("assets/objects/ab/not-a-hash"), scope), "pass");
  });

  it("serves the page from the network, with a copy for offline starts", () => {
    for (const path of ["", "index.html", "boot.js", "build.json", "asset-index.json"]) {
      assert.equal(strategyFor(at(path), scope), "page", path);
    }
  });

  it("leaves the client jar to the storage worker, and anything else alone", () => {
    assert.equal(strategyFor(at("client.jar"), scope), "pass");
    assert.equal(strategyFor(at("shaders.json"), scope), "pass");
    assert.equal(strategyFor(new URL("https://elsewhere.test/incari/boot.js"), scope), "pass");
    assert.equal(strategyFor(new URL("https://example.test/other/boot.js"), scope), "pass");
    assert.equal(strategyFor(at("build.json"), scope, "POST"), "pass");
  });

  it("leaves the Rust build to its own worker, versioned files and page included", () => {
    assert.equal(strategyFor(at("rust/pkg/horizon-client_bg.wasm?v=0123456789abcdef"), scope), "pass");
    assert.equal(strategyFor(at("rust/index.html"), scope), "pass");
  });
});

describe("staleBuildEntries", () => {
  it("drops every cached build file the running build doesn't load", () => {
    const cached = [at("incari.wasm?v=old").href, at("incari.wasm?v=new").href, at("incari.pack?v=new").href];
    assert.deepEqual(staleBuildEntries(cached, ["incari.wasm?v=new", "incari.pack?v=new"], scope), [
      at("incari.wasm?v=old").href,
    ]);
  });
});
