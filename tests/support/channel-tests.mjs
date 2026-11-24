import assert from "node:assert/strict";
import { webcrypto } from "node:crypto";
import { afterEach, test } from "node:test";

// The caller supplies its own installed Tauri API, so these tests exercise the
// same generated runtime against v2 and v3 without accidentally importing v2.
//
// Each test builds a fresh channel per command call. Tauri gives every command
// invocation its own message index sequence and end signal for the same callback
// id, so reusing one channel across invocations loses messages.
export function channelTests({ Channel, mockIPC, clearMocks }, ts, js) {
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

    test(`${name} semantic channels transform handlers set before the call`, async () => {
      mockIPC(() => {});
      const OriginalURL = globalThis.URL;
      let transforms = 0;
      globalThis.URL = class extends OriginalURL {
        constructor(value) { super(value); transforms++; }
      };
      try {
        const channel = new Channel();
        const values = [];
        channel.onmessage = (payload) => values.push(payload.url);
        await api.commands.streamUrls(channel);
        window.__TAURI_INTERNALS__.callbacks.get(channel.id)({
          index: 0, message: { url: "https://example.com/" },
        });
        assert.equal(transforms, 1);
        assert.equal(values[0].hostname, "example.com");
      } finally {
        globalThis.URL = OriginalURL;
      }
    });

    test(`${name} each command call uses its own channel mapping`, async () => {
      mockIPC(() => {});
      const values = [];
      const mapped = new Channel((value) => values.push(value.url));
      await api.commands.streamUrls(mapped);
      window.__TAURI_INTERNALS__.callbacks.get(mapped.id)({
        index: 0, message: { url: "https://example.com/" },
      });
      assert.ok(values[0] instanceof URL);

      const plain = new Channel((value) => values.push(value.url));
      await api.commands.plainUrls(plain);
      window.__TAURI_INTERNALS__.callbacks.get(plain.id)({
        index: 0, message: { url: "https://example.com/" },
      });
      assert.equal(values[1], "https://example.com/");
    });
  }
}
