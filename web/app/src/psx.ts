import init, { PsxWebEmulator, InitOutput } from "../../pkg/rsx_redux_web"
import wasmData from '../../pkg/rsx_redux_web_bg.wasm'
import { Joypad } from "./input/joypad"
import { AudioOutput } from "./output/audio_output"
import { VideoOutput } from "./output/video_output"
import { WaveVisualizer } from "./util/wave_visualizer"

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
    private audioOutput: AudioOutput|null = null

    private biosReady = false
    private gameReady = false
    private joypad = new Joypad()
    private waveVisualizer = new WaveVisualizer()
    private isPaused = true
    private isRunning = false

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

        this.initializeEmulator()
    }

    togglePause() {
        if (this.isRunning) {
            const pause = document.getElementById('nav-pause')!
            const pauseButton = document.getElementById('btn-pause')!

            if (!this.isPaused) {
                console.log(pause.children[0].innerHTML)
                console.log(pause.children[0].children)
                pause.children[0].innerHTML = `<i class="fa-solid fa-play"></i>`
                pause.children[1].textContent = 'Resume'
                pauseButton.children[0].innerHTML = `<i class="fa-solid fa-play"></i>`
                cancelAnimationFrame(this.frameNumber)
            } else {
                console.log(pause.children)
                pause.children[0].innerHTML = `<i class="fa-solid fa-pause"></i>`
                pause.children[1].textContent = 'Pause'
                pauseButton.children[0].innerHTML = `<i class="fa-solid fa-pause"></i>`
                this.frameNumber = requestAnimationFrame((time) => this.runFrame(time))
            }

            this.isPaused = !this.isPaused
        }
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
        this.biosReady = true
        document.getElementById('btn-load-game')!.removeAttribute('disabled')
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
        if (!this.biosReady) {
            return
        }
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

    undoMappings() {
        this.joypad.undoMappings()
    }

    saveMappings() {
        this.joypad.saveMappings()
    }

    remapKey(el: HTMLElement) {
        this.joypad.remapKey(el)
    }

    openControllerModal() {
        this.joypad.openControllerModal()
    }

    async handleGameFile(gameFile: File) {
        this.isPaused = false
        this.isRunning = true

        const data = await this.readFile(gameFile)

        const gameBytes = new Uint8Array(data)

        cancelAnimationFrame(this.frameNumber)

        this.emulator!.load_rom(gameBytes)

        const placeholder = document.getElementById('placeholder')

        if (placeholder != null) {
            placeholder.remove()
            const canvas = document.createElement('canvas')

            canvas.setAttribute('width', '640');
            canvas.setAttribute('height', '480')

            document.getElementById('display')!.append(canvas)
            this.videoOutput = new VideoOutput(canvas, this.emulator!, this.wasm!)
        } else {
            this.emulator!.reset()
        }

        this.audioOutput = new AudioOutput(this.emulator!, this.wasm!)
        this.paused = false

        document.getElementById('status-dot')!.classList.add('is-active')
        document.getElementById('status-text')!.innerText = 'Game running'

        this.joypad.updateButtonMap()
        this.joypad.addKeyboardControllerListeners()
        this.enableSwapDisc()

        this.joypad.setEmulator(this.emulator)

        this.frameNumber = requestAnimationFrame((time) => {
            this.runFrame(time)
        })
    }

    reset() {
        if (this.emulator != null) {
            cancelAnimationFrame(this.frameNumber)
            this.emulator.reset()

            this.frameNumber = requestAnimationFrame((time) => this.runFrame(time))
        }
    }

    resetToDefaults() {
        this.joypad.resetToDefaults()
    }

    enableSwapDisc() {
        this.gameReady = true
        const swapDisc = document.getElementById('btn-swap-disc')
        swapDisc?.removeAttribute('disabled')
    }

    toggleWaveform() {
        this.waveVisualizer.toggle()
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
                const samples = this.audioOutput?.pushSamples()

                this.waveVisualizer.plot(samples!)

                this.joypad.handleInputAndVibration()
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
        if (!this.gameReady) {
            return
        }
    }
}