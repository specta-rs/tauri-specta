import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { afterEach, test } from "node:test";

/**
 * `Channel.deserialize` is added in tauri-apps/tauri#16583 and is not in a published
 * `@tauri-apps/api` yet, so the generated bindings can't run against the version these
 * tests install. Stand in for it with the same semantics until the minimum supported
 * API version has the method, then delete this.
 */
function installChannelDeserialize(Channel) {
  if (typeof Channel.prototype.deserialize === "function") return;

  const state = Symbol("tauriSpectaChannelTransform");
  Channel.prototype.deserialize = function (transform) {
    const existing = this[state];
    if (existing) {
      // Replacing must not stack transforms: a second generated module mapping the
      // same channel still runs one transform per message.
      existing.transform = transform;
      return this;
    }

    const current = { handler: this.onmessage, transform };
    // The class delivers to its private handler, so point that at the transform and
    // back the public accessor with `current.handler` so handlers can still be set
    // (and replaced) after registration.
    this.onmessage = (payload) => current.handler.call(this, current.transform(payload));
    Object.defineProperty(this, "onmessage", {
      configurable: true,
      get: () => current.handler,
      set: (handler) => {
        current.handler = handler;
      },
    });
    Object.defineProperty(this, state, { value: current });
    return this;
  };
}

// The caller supplies its own installed Tauri API, so these tests exercise the
// same generated runtime against v2 and v3 without accidentally importing v2.
export function channelTests({ Channel, mockIPC, clearMocks }, ts, js) {
  installChannelDeserialize(Channel);
  globalThis.window = { crypto: webcrypto };
  afterEach(() => clearMocks());

  for (const [name, api] of [["TypeScript", ts], ["JSDoc", js]]) {
    test(`${name} semantic channels preserve id, ordering, and cleanup`, async () => {
      let sent;
      mockIPC((_command, args) => { sent = args.channel; });
      const values = [];
      const channel = new Channel((payload) => values.push(payload.url));
      const id = channel.id;
      const wireId = channel.toJSON();
      await api.commands.streamUrls(channel);
      assert.equal(sent, channel);
      assert.equal(channel.id, id);
      assert.equal(channel.toJSON(), wireId);
      assert.equal(window.__TAURI_INTERNALS__.callbacks.size, 1);

      const send = window.__TAURI_INTERNALS__.callbacks.get(id);
      send({ index: 1, message: { url: "https://second.example/" } });
      send({ index: 0, message: { url: "https://first.example/" } });
      assert.deepEqual(values.map((url) => url.hostname), ["first.example", "second.example"]);
      assert.ok(values.every((url) => url instanceof URL));
      send({ index: 2, end: true });
      assert.equal(window.__TAURI_INTERNALS__.callbacks.size, 0);
    });

    test(`${name} semantic channels allow late handlers and remapping without stacking`, async () => {
      mockIPC(() => {});
      const OriginalURL = globalThis.URL;
      let transforms = 0;
      globalThis.URL = class extends OriginalURL {
        constructor(value) { super(value); transforms++; }
      };
      try {
        const channel = new Channel();
        await api.commands.streamUrls(channel);
        const values = [];
        const handler = (payload) => values.push(payload.url);
        channel.onmessage = handler;
        assert.equal(channel.onmessage, handler);
        await api.commands.streamUrls(channel);
        // A different generated module must also replace, not stack, transforms.
        await (api === ts ? js : ts).commands.streamUrls(channel);
        window.__TAURI_INTERNALS__.callbacks.get(channel.id)({
          index: 0, message: { url: "https://example.com/" },
        });
        assert.equal(transforms, 1);
        assert.equal(values[0].hostname, "example.com");
        assert.equal(window.__TAURI_INTERNALS__.callbacks.size, 1);
      } finally {
        globalThis.URL = OriginalURL;
      }
    });

    test(`${name} mapped channels retain their mapping; plain contracts need a fresh channel`, async () => {
      mockIPC(() => {});
      const values = [];
      const mapped = new Channel((value) => values.push(value.url));
      await api.commands.streamUrls(mapped);
      // Deliberately mixing contracts demonstrates the documented restriction:
      // a plain command does not reset the mapping on an existing channel.
      await api.commands.plainUrls(mapped);
      window.__TAURI_INTERNALS__.callbacks.get(mapped.id)({
        index: 0, message: { url: "https://example.com/" },
      });
      assert.ok(values[0] instanceof URL);

      const fresh = new Channel((value) => values.push(value.url));
      await api.commands.plainUrls(fresh);
      window.__TAURI_INTERNALS__.callbacks.get(fresh.id)({
        index: 0, message: { url: "https://example.com/" },
      });
      assert.equal(values[1], "https://example.com/");
    });
  }
}
