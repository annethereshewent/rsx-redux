import { PsxWebEmulator } from "../../../pkg/rsx_redux_web"

export class Joypad {
    private emulator: PsxWebEmulator

    constructor(emulator: PsxWebEmulator) {
        this.emulator = emulator
    }

    async handleInput() {
        const gamepad = navigator.getGamepads()[0]
        console.log(gamepad)
        if (gamepad != null) {
            console.log(gamepad.buttons)
        }
    }
}