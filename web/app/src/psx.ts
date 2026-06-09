import init, { PsxWebEmulator, InitOutput } from "../../pkg/rsx_redux_web"
import wasmData from '../../pkg/rsx_redux_web_bg.wasm'
import { VideoOutput } from "./output/video_output"

const FPS_INTERVAL = 1000 / 60

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

                return
            }

            if (action && action in this) {
                (this as any)[action]()
            }
        })

        this.initializeEmulator()
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

    async initWasm() {
        console.log('initialized emulator!')
        this.wasm = await init(wasmData)
        this.emulator = new PsxWebEmulator()
        console.log(this.emulator)
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