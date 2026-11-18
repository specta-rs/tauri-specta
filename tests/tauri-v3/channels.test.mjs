import { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { channelTests } from "../support/channel-tests.mjs";
import * as ts from "./target/bindings/semantic.ts";
import * as js from "./target/bindings/semantic.js";

channelTests({ Channel, clearMocks, mockIPC }, ts, js);
