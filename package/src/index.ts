import { invoke } from "@tauri-apps/api";

type CommandDef = {
  name: string;
  input: Record<string, unknown> | null;
  result: any;
};

export function typedInvoke<TCommands extends CommandDef>() {
  return {
    invoke: <K extends TCommands["name"]>(
      key: K,
      input: Extract<TCommands, { name: K }>["input"]
    ): Promise<Extract<TCommands, { name: K }>["result"]> =>
      invoke(key, input || undefined),
  };
}
