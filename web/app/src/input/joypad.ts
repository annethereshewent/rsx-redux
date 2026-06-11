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

const DEFAULT_KEY_MAP = new Map<string, string>([
    ['select', 'tab'],
    ['l3', 'z'],
    ['r3', 'm'],
    ['start', 'enter'],
    ['up', 'w'],
    ['right', 'd'],
    ['down', 's'],
    ['left', 'a'],
    ['l2', '7'],
    ['r2', '9'],
    ['l1', 'u'],
    ['r1', 'o'],
    ['triangle', 'i'],
    ['circle', 'l'],
    ['cross', 'k'],
    ['square', 'j']
])

const keyToCode = new Map<string, number>([
    ["select", 0],
    ["l3", 1],
    ["r3", 2],
    ["start", 3],
    ["up", 4],
    ["right", 5],
    ["down", 6],
    ["left", 7],
    ["l2", 8],
    ["r2", 9],
    ["l1", 10],
    ["r1", 11],
    ["triangle", 12],
    ["circle", 13],
    ["cross", 14],
    ["square", 15]
])

export class Joypad {
    private emulator: PsxWebEmulator|null = null
    private gamepadIndex = 0
    private previousKeyMap = new Map()
    private buttonMap = new Map<string, number>()
    private keyMap = new Map<string, string>([
        ['select', 'tab'],
        ['l3', 'z'],
        ['r3', 'm'],
        ['start', 'enter'],
        ['up', 'w'],
        ['right', 'd'],
        ['down', 's'],
        ['left', 'a'],
        ['l2', '7'],
        ['r2', '9'],
        ['l1', 'u'],
        ['r1', 'o'],
        ['triangle', 'i'],
        ['circle', 'l'],
        ['cross', 'k'],
        ['square', 'j']
    ])


    constructor() {
        window.addEventListener('gamepadconnected', (e) => {
            this.gamepadIndex = e.gamepad.index
        })

        const savedKeyMap = JSON.parse(localStorage.getItem('psx-keyboard-mappings') || 'null')

        if (savedKeyMap != null) {
            this.keyMap = new Map(savedKeyMap)
        }
    }

    private controllerClickListener = (event: Event) => {
        const modal = document.getElementById('controller-modal')
        const modalBox = modal?.children[0]
        if (!modalBox?.contains((event.target as HTMLElement)!) && modal?.classList.contains('is-active')) {
            this.undoMappings()
        }
    }

    undoMappings() {
        this.keyMap = new Map(this.previousKeyMap)
        this.closeModal()
    }

    saveMappings() {
        localStorage.setItem('psx-keyboard-mappings', JSON.stringify(Array.from(this.keyMap.entries())))

        this.closeModal()
        this.updateButtonMap()
    }

    openControllerModal() {
        this.updateBindings()

        const modal = document.getElementById('controller-modal')

        document.removeEventListener('click', this.controllerClickListener)

        modal?.classList.add('is-active')

        this.previousKeyMap = new Map(this.keyMap)

        document.addEventListener('click', this.controllerClickListener)
    }

    updateBindings() {
        this.keyMap.forEach((value, key, _map) => {
            const element = document.getElementById(`button-${key}`)

            if (element != null) {
                element.innerText = this.formattedKey(value)
            }
        })
    }

    resetToDefaults() {
        this.keyMap = new Map(DEFAULT_KEY_MAP)
        this.closeModal()
        this.updateButtonMap()
    }

    closeModal() {
        const modal = document.getElementById('controller-modal')
        modal?.classList.remove('is-active')
        document.removeEventListener('click', this.controllerClickListener)
    }

    addKeyboardControllerListeners() {
        document.addEventListener('keydown', (e) => {
            const buttonCode = this.buttonMap.get(e.key.toLowerCase().replace('arrow', ''))

            if (buttonCode != null) {
                e.preventDefault()

                this.emulator!.update_input(buttonCode, true)
            }
        })

        document.addEventListener('keyup', (e) => {
            const buttonCode = this.buttonMap.get(e.key.toLowerCase().replace('arrow', ''))

            if (buttonCode != null) {
                e.preventDefault()

                this.emulator!.update_input(buttonCode, false)
            }
        })
    }

    setEmulator(emulator: PsxWebEmulator|null) {
        this.emulator = emulator
    }

    remapKey(el: HTMLElement) {
        const previousKey = el.innerText.toLowerCase().replace('arrow', '')
        el.innerText = "Listening...."
        const remapListener = (ev: KeyboardEvent) => {
            ev.preventDefault()
            const selectedKey = ev.key.toLowerCase().replace('arrow', '')

            this.keyMap.forEach((value, key, map) => {
                if (value.toLowerCase() == selectedKey) {
                    map.set(key, previousKey)
                    document.getElementById(`button-${key}`)!.innerText = this.formattedKey(previousKey)
                }
            })

            el.innerText = this.formattedKey(selectedKey)

            this.keyMap.set(el.dataset.button!, selectedKey)

            el.removeEventListener('keydown', remapListener)
        }
        el.removeEventListener('keydown', remapListener)
        el.addEventListener('keydown', remapListener)
    }

    private formattedKey(key: string) {
        return key.charAt(0).toUpperCase() + key.slice(1, key.length).toLowerCase()
    }

    updateButtonMap() {
        this.buttonMap = new Map()

        this.keyMap.forEach((value, key, map) => {
            const entry = keyToCode.get(key)
            if (entry != null) {
                this.buttonMap.set(value, entry)
            }
        })
    }

    async handleInputAndVibration() {
        const gamepad = navigator.getGamepads()[this.gamepadIndex]

        if (gamepad != null && this.emulator != null) {
            this.handleVibration(gamepad)

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

    handleVibration(gamepad: Gamepad) {
        if (this.emulator != null) {
            let [smallMotor, largeMotor] = this.emulator.get_rumble()

            smallMotor *= 0.2
            largeMotor /= 255

            gamepad.vibrationActuator.playEffect("dual-rumble", {
                startDelay: 0,
                duration: 200,
                weakMagnitude: smallMotor,
                strongMagnitude: largeMotor,
            })
        }
    }
}