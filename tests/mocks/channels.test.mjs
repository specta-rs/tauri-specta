import { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { channelTests } from "../support/channel-tests.mjs";
import * as ts from "../snapshots/semantic_channels_ts/bindings.ts";
import * as js from "../snapshots/semantic_channels_jsdoc/bindings.js";

channelTests({ Channel, clearMocks, mockIPC }, ts, js);
