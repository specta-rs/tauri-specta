import { invoke } from "@tauri-apps/api";

type CommandDef = {
  name: string;
  args: Record<string, unknown> | null;
  result: any;
};

export function typedInvoke<TCommands extends CommandDef>() {
  return {
    invoke: <K extends TCommands["name"]>(
      key: K,
      args: Extract<TCommands, { name: K }>["args"]
    ): Promise<Extract<TCommands, { name: K }>["result"]> =>
      invoke(key, args || undefined),
  };
}
