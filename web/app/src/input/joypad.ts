import { PsxWebEmulator } from "../../../pkg/rsx_redux_web"

enum GamepadButtons {
    Cross = 0,
    Circle = 1,
    Square = 2,
    Triangle = 3,
    L1 = 4,
    R1 = 5,
    L2 = 6,
    R2 = 7,
    Select = 8,
    Start = 9,
    LeftStick = 10,
    RightStick = 11,
    Up = 12,
    Down = 13,
    Left = 14,
    Right = 15,
    Touchpad = 17
}

enum PsxButtons {
    SELECT = 0,
    L3 = 1,
    R3 = 2,
    START = 3,
    UP = 4,
    RIGHT = 5,
    DOWN = 6,
    LEFT = 7,
    L2 = 8,
    R2 = 9,
    L1 = 10,
    R1 = 11,
    TRIANGLE = 12,
    CIRCLE = 13,
    CROSS = 14,
    SQUARE = 15,
}

export class Joypad {
    private emulator: PsxWebEmulator
    private gamepadIndex = 0

    constructor(emulator: PsxWebEmulator) {
        this.emulator = emulator

        window.addEventListener('gamepadconnected', (e) => {
            this.gamepadIndex = e.gamepad.index
        })
    }

    async handleInput() {
        const gamepad = navigator.getGamepads()[this.gamepadIndex]
        if (gamepad != null) {
            this.emulator.update_input(PsxButtons.SELECT, gamepad.buttons[GamepadButtons.Select].pressed)
            this.emulator.update_input(PsxButtons.L3, gamepad.buttons[GamepadButtons.LeftStick].pressed)
            this.emulator.update_input(PsxButtons.R3, gamepad.buttons[GamepadButtons.RightStick].pressed)
            this.emulator.update_input(PsxButtons.START, gamepad.buttons[GamepadButtons.Start].pressed)
            this.emulator.update_input(PsxButtons.UP, gamepad.buttons[GamepadButtons.Up].pressed)
            this.emulator.update_input(PsxButtons.RIGHT, gamepad.buttons[GamepadButtons.Right].pressed)
            this.emulator.update_input(PsxButtons.DOWN, gamepad.buttons[GamepadButtons.Down].pressed)
            this.emulator.update_input(PsxButtons.LEFT, gamepad.buttons[GamepadButtons.Left].pressed)
            this.emulator.update_input(PsxButtons.L2, gamepad.buttons[GamepadButtons.L2].pressed)
            this.emulator.update_input(PsxButtons.R2, gamepad.buttons[GamepadButtons.R2].pressed)
            this.emulator.update_input(PsxButtons.L1, gamepad.buttons[GamepadButtons.L1].pressed)
            this.emulator.update_input(PsxButtons.R1, gamepad.buttons[GamepadButtons.R1].pressed)
            this.emulator.update_input(PsxButtons.TRIANGLE, gamepad.buttons[GamepadButtons.Triangle].pressed)
            this.emulator.update_input(PsxButtons.CIRCLE, gamepad.buttons[GamepadButtons.Circle].pressed)
            this.emulator.update_input(PsxButtons.CROSS, gamepad.buttons[GamepadButtons.Cross].pressed)
            this.emulator.update_input(PsxButtons.SQUARE, gamepad.buttons[GamepadButtons.Square].pressed)

            if (gamepad.buttons[GamepadButtons.Touchpad].pressed) {
                this.emulator.toggle_digital_mode()
            }

            let leftX = this.normalizeAxis(gamepad.axes[0])
            let leftY = this.normalizeAxis(gamepad.axes[1])
            let rightX = this.normalizeAxis(gamepad.axes[2])
            let rightY = this.normalizeAxis(gamepad.axes[3])

            this.emulator.set_left_thumbstick(leftX, leftY)
            this.emulator.set_right_thumbstick(rightX, rightY)
        }
    }

    normalizeAxis(axis: number): number {
        let normalized = axis
        if (normalized < 0) {
            normalized = -normalized * -32768
        } else {
            normalized = normalized * 32767
        }

        normalized >>= 8

        normalized += 128

        return normalized
    }
}