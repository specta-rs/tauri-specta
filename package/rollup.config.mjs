import typescript from "@rollup/plugin-typescript";
import { resolve } from "path";

export default {
  input: "src/index.ts",
  output: [
    {
      format: "cjs",
      file: resolve(`dist/cjs/index.js`),
      sourcemap: true,
    },
    {
      format: "esm",
      file: resolve(`dist/esm/index.js`),
      sourcemap: true,
    },
  ],
  format: ["esm", "cjs"],
  external: ["@tauri-apps/api"],
  plugins: [typescript()],
};
