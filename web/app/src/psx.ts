import init, { PsxWebEmulator, InitOutput } from "../../pkg/rsx_redux_web"
import wasmData from '../../pkg/rsx_redux_web_bg.wasm'
import { VideoOutput } from "./output/video_output"

const FPS_INTERVAL = 1000 / 60

const keyToCode = {
    "select": 0,
    "l3": 1,
    "r3": 2,
    "start": 3,
    "up": 4,
    "right": 5,
    "down": 6,
    "left": 7,
    'l2': 8,
    'r2': 9,
    "l1": 10,
    "r1": 11,
    "triangle": 12,
    "circle": 13,
    "cross": 14,
    "square": 15
}

export class Psx {
    private wasm: InitOutput|null = null
    private emulator: PsxWebEmulator|null = null
    private frameNumber = -1
    private previousTime = 0
    private realPreviousTime = 0
    private paused = true
    private fps = 0
    private frames = 0
    private videoOutput: VideoOutput|null = null
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
    private previousKeyMap = new Map()

    private controllerClickListener = (event: Event) => {
        const modal = document.getElementById('controller-modal')
        const modalBox = modal?.children[0]
        if (!modalBox?.contains((event.target as HTMLElement)!) && modal?.classList.contains('is-active')) {
            this.undoMappings()
        }
    }

    constructor() {
        document.addEventListener("click", (e) => {
            const el = (e.target as HTMLElement).closest('[data-action]')

            if (!el) {
                return
            }

            const action = (el as HTMLElement).dataset.action

            if (action == 'toggle') {
                const target = document.getElementById((el as HTMLElement).dataset.target!)

                target?.classList.toggle('is-active')

                if (target?.classList.contains('is-active')) {
                    const clickListener = (event: Event) => {
                        const modals = document.getElementsByClassName('modal-box')

                        for (const modal of modals) {
                            if (!modal.contains((event.target as HTMLElement)!) && modal.parentElement?.classList.contains('is-active')) {
                                modal.parentElement?.classList.remove('is-active')
                                document.removeEventListener('click', clickListener)
                                return
                            }
                        }
                    }

                    document.addEventListener('click', clickListener)
                }

                return
            }

            if (action && action in this) {
                (this as any)[action](el)
            }
        })

        const savedKeyMap = JSON.parse(localStorage.getItem('psx-keyboard-mappings') || 'null')

        if (savedKeyMap != null) {
            this.keyMap = new Map(savedKeyMap)
            this.updateBindings()
        }

        this.initializeEmulator()
    }

    updateBindings() {
        this.keyMap.forEach((value, key, _map) => {
            const element = document.getElementById(`button-${key}`)

            if (element != null) {
                element.innerText = this.formattedKey(value)
            }
        })
    }

    openControllerModal() {
        this.updateBindings()

        const modal = document.getElementById('controller-modal')

        document.removeEventListener('click', this.controllerClickListener)

        modal?.classList.add('is-active')

        this.previousKeyMap = new Map(this.keyMap)

        if (modal?.classList.contains('is-active')) {
            document.addEventListener('click', this.controllerClickListener)
        }
    }

    undoMappings() {
        this.keyMap = new Map(this.previousKeyMap)
        const modal = document.getElementById('controller-modal')
        modal?.classList.remove('is-active')
        document.removeEventListener('click', this.controllerClickListener)
    }

    saveMappings() {
        localStorage.setItem('psx-keyboard-mappings', JSON.stringify(Array.from(this.keyMap.entries())))

        const modal = document.getElementById('controller-modal')
        modal?.classList.remove('is-active')
        document.removeEventListener('click', this.controllerClickListener)
    }

    async initializeEmulator() {
        await this.initWasm()

        const biosDataArr = JSON.parse(localStorage.getItem('psx-bios') || 'null')

        if (biosDataArr != null) {
            const biosBytes = new Uint8Array(biosDataArr)

            this.emulator!.load_bios(biosBytes)

            this.enableGameButton()
        }
    }

    enableGameButton() {
        document.getElementById('status-text')!.innerText = 'BIOS loaded'
        document.getElementById('btn-load-game')!.removeAttribute('disabled')
    }

    private formattedKey(key: string) {
        return key.charAt(0).toUpperCase() + key.slice(1, key.length).toLowerCase()
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

    async initWasm() {
        this.wasm = await init(wasmData)
        this.emulator = new PsxWebEmulator()
    }

    loadBios() {
        const biosInput = document.getElementById('file-bios') as HTMLInputElement

        if (biosInput != null) {
            biosInput.onchange = (e) => {
                const files = (e.target as HTMLInputElement)?.files

                if (files != null) {
                    const file = files[0]

                    this.handleBiosFile(file)
                }
            }
            biosInput.click()
        }
    }

    loadGame() {
        const gameInput = document.getElementById('file-game')

        if (gameInput != null) {
            gameInput.onchange = (e) => {
                const files = (e.target as HTMLInputElement)?.files

                if (files != null) {
                    const file = files[0]

                    this.handleGameFile(file)
                }
            }
            gameInput.click()
        }
    }

    async handleGameFile(gameFile: File) {
        const data = await this.readFile(gameFile)

        const gameBytes = new Uint8Array(data)

        this.emulator!.load_rom(gameBytes)

        const canvas = document.createElement('canvas')

        canvas.setAttribute('width', '640');
        canvas.setAttribute('height', '480')

        document.getElementById('placeholder')!.remove()
        document.getElementById('display')!.append(canvas)

        this.videoOutput = new VideoOutput(canvas, this.emulator!, this.wasm!)
        this.paused = false

        document.getElementById('status-dot')!.classList.add('is-active')
        document.getElementById('status-text')!.innerText = 'Game running'

        this.frameNumber = requestAnimationFrame((time) => {
            this.runFrame(time)
        })
    }

    runFrame(time: number) {
        const diff = time - this.previousTime

        if (!this.paused) {
            const realDiff = time - this.realPreviousTime

            this.fps = Math.floor(1000 / realDiff)

            if (this.frames == 60) {
                this.frames = 0

                this.updateFps()
            }

            this.realPreviousTime = time
            if (diff >= FPS_INTERVAL || this.previousTime == 0) {
                this.emulator!.step_frame()
                this.videoOutput?.updateCanvas()
            }

            this.previousTime = time - (diff % FPS_INTERVAL)
            this.frames++
            this.frameNumber = requestAnimationFrame((time) => this.runFrame(time))
        }

    }

    updateFps() {
        document.getElementById('status-fps')!.innerText = `${this.fps} FPS`
    }

    async handleBiosFile(biosFile: File) {
        const dataArrayBuffer = await this.readFile(biosFile)

        const biosBytes = new Uint8Array(dataArrayBuffer)

        this.emulator?.load_bios(biosBytes)

        this.enableGameButton()

        localStorage.setItem('psx-bios', JSON.stringify(Array.from(biosBytes)))
    }

    readFile(file: File): Promise<ArrayBuffer> {
        const fileReader = new FileReader()

        fileReader.readAsArrayBuffer(file)

        return new Promise((resolve, reject) => {
            fileReader.onload = (e) => {
                resolve(fileReader.result as ArrayBuffer)
            }

            fileReader.onerror = (e) => {
                fileReader.abort()
                reject(new Error('error parsing file'))
            }
        })
    }

    swapDisc() {

    }
}