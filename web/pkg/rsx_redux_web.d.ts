/* tslint:disable */
/* eslint-disable */

export class PsxWebEmulator {
    free(): void;
    [Symbol.dispose](): void;
    drain_samples(): Int16Array;
    get_digital_mode(): boolean;
    get_dimensions(): Uint32Array;
    get_memory_bytes(): Uint8Array | undefined;
    get_rumble(): Uint8Array;
    load_bios(bios_bytes: Uint8Array): void;
    load_rom(game_bytes: Uint8Array): void;
    load_state(data: Uint8Array): void;
    constructor(canvas_id: string);
    reset(): void;
    save_state(): Uint8Array;
    set_digital_mode(mode: boolean): void;
    set_exe(exe_bytes?: Uint8Array | null): void;
    set_left_thumbstick(normalized_x: number, normalized_y: number): void;
    set_left_x(value: number): void;
    set_left_y(value: number): void;
    set_memory_card(memory_bytes: Uint8Array): void;
    set_right_thumbstick(normalized_x: number, normalized_y: number): void;
    step_frame(): void;
    switch_selected_controller(controller_id: number): void;
    toggle_digital_mode(): void;
    update_input(button: number, pressed: boolean): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_psxwebemulator_free: (a: number, b: number) => void;
    readonly psxwebemulator_new: (a: number, b: number) => number;
    readonly psxwebemulator_load_bios: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_load_rom: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_step_frame: (a: number) => void;
    readonly psxwebemulator_drain_samples: (a: number) => [number, number];
    readonly psxwebemulator_update_input: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_toggle_digital_mode: (a: number) => void;
    readonly psxwebemulator_set_left_thumbstick: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_set_right_thumbstick: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_set_left_x: (a: number, b: number) => void;
    readonly psxwebemulator_set_left_y: (a: number, b: number) => void;
    readonly psxwebemulator_set_memory_card: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_load_state: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_save_state: (a: number) => [number, number];
    readonly psxwebemulator_get_dimensions: (a: number) => [number, number];
    readonly psxwebemulator_set_exe: (a: number, b: number, c: number) => void;
    readonly psxwebemulator_get_rumble: (a: number) => [number, number];
    readonly psxwebemulator_get_digital_mode: (a: number) => number;
    readonly psxwebemulator_set_digital_mode: (a: number, b: number) => void;
    readonly psxwebemulator_switch_selected_controller: (a: number, b: number) => void;
    readonly psxwebemulator_reset: (a: number) => void;
    readonly psxwebemulator_get_memory_bytes: (a: number) => [number, number];
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
