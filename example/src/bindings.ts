// This file was generated by [tauri-specta](https://github.com/oscartbeaumont/tauri-specta). Do not edit this file manually.

declare global {
    interface Window {
        __TAURI_INVOKE__<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
    }
}

const invoke = window.__TAURI_INVOKE__;

export function helloWorld(myName: string) {
    return invoke<string>("hello_world", { myName })
}

export function goodbyeWorld() {
    return invoke<string>("goodbye_world")
}

export function someStruct() {
    return invoke<MyStruct>("some_struct")
}

export type MyStruct = { some_field: string }
