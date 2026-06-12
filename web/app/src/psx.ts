import init, { PsxWebEmulator, InitOutput } from "../../pkg/rsx_redux_web"
import wasmData from '../../pkg/rsx_redux_web_bg.wasm'
import { Joypad } from "./input/joypad"
import { AudioOutput } from "./output/audio_output"
import { VideoOutput } from "./output/video_output"
import { RsxDb, SaveState } from "./saves/rsx_db"
import { StateManager } from "./saves/state_manager"
import { WaveVisualizer } from "./util/wave_visualizer"

const FPS_INTERVAL = 1000 / 60
const MEMORY_CARD_SIZE = 0x20000

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
    private audioOutput = new AudioOutput()

    private biosReady = false
    private gameReady = false
    private joypad = new Joypad()
    private waveVisualizer = new WaveVisualizer()
    private isPaused = true
    private isRunning = false
    private memoryCard = "memory_card1"
    private memoryCardData = new Uint8Array(MEMORY_CARD_SIZE)
    private rsxDb = new RsxDb()
    private stateManager: StateManager|null = null

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

        const card = localStorage.getItem('psx-memory-card')

        if (card != null) {
            const dropdown = document.getElementById('memory-card-select') as HTMLSelectElement

            dropdown.value = card

            this.memoryCard = card
        }

        this.loadMemoryCard()

        document.getElementById('memory-card-select')!.addEventListener('change', (ev) => {
            const memoryCard = (ev.target as HTMLSelectElement).value

            this.setMemoryCard(memoryCard)
        })

        document.addEventListener('savestatemodalchange', () => {
            this.togglePause()
        })

        document.addEventListener('keydown', async (ev) => {
            if (ev.key.toLowerCase() == 'f5') {
                ev.preventDefault()
                const imageUrl = this.getImageUrl()
                const state = await this.stateManager?.createSaveState(0, imageUrl)

                if (state != null) {
                    this.stateManager?.updateStateMenuListItem(0, state)
                    this.stateManager?.updateStateModalEntry(0, state)
                }
            } else if (ev.key.toLowerCase() == 'f7') {
                ev.preventDefault()
                const data = await this.stateManager?.loadSaveState(0)

                if (data != null) {
                    this.emulator!.load_state(data)
                }
            }
        })

        this.initializeEmulator()
    }

    async loadMemoryCard() {
        const data = await this.rsxDb.getMemoryCard(this.memoryCard)

        if (data != null) {
            this.memoryCardData = data as Uint8Array<ArrayBuffer>
            document.getElementById('mem-card-status')!.textContent = 'Saves found'
        } else {
            this.memoryCardData = new Uint8Array(MEMORY_CARD_SIZE)
            document.getElementById('mem-card-status')!.textContent = 'No saves'
        }

        this.emulator?.set_memory_card(this.memoryCardData)
    }

    setMemoryCard(card: string) {
        localStorage.setItem('psx-memory-card', card)
        this.memoryCard = card
        this.loadMemoryCard()
    }

    toggleFullscreen() {
        if (document.fullscreenElement == null) {
            document.documentElement.requestFullscreen()
        } else {
            document.exitFullscreen()
        }
    }

    openSaveStatesModal() {
        this.stateManager?.openSaveStatesModal()
    }

    closeSaveStatesModal() {
        this.stateManager?.closeModal()
    }

    async saveState(el: HTMLElement) {
        const index = el.dataset.slot == 'quick' ? 0 : parseInt(el.dataset.slot || "0")
        const imageUrl = this.getImageUrl()

        this.stateManager!.createSaveState(index, imageUrl)
    }

    async loadState(el: HTMLElement) {
        const index = el.dataset.slot == 'quick' ? 0 : parseInt(el.dataset.slot || "0")

        const data = await this.stateManager!.loadSaveState(index)

        if (data != null) {
            this.emulator!.load_state(data)
        }
    }

    async deleteState(el: HTMLElement) {
        const index = el.dataset.slot == 'quick' ? 0 : parseInt(el.dataset.slot || "0")

        this.stateManager!.deleteState(index)
    }

    getImageUrl() {
        const memory = new Uint8Array(this.wasm!.memory.buffer, this.emulator!.get_framebuffer(), this.emulator!.get_framebuffer_size())
        const [width, height] = this.emulator!.get_dimensions()

        const canvas = document.getElementById('save-state-canvas')! as HTMLCanvasElement

        canvas.setAttribute('width', `${width}`)
        canvas.setAttribute('height', `${height}`)

        const context = canvas.getContext('2d')

        const imageData = context!.getImageData(0, 0, width, height)

        for (let y = 0; y < width; y++) {
            for (let x = 0; x < height; x++) {
                const index = x * 3 + y * height * 3
                const canvasIndex = x * 4 + y * height * 4

                imageData.data[canvasIndex] = memory[index]
                imageData.data[canvasIndex + 1] = memory[index + 1]
                imageData.data[canvasIndex + 2] = memory[index + 2]
                imageData.data[canvasIndex + 3] = 255
            }
        }

        context!.putImageData(imageData, 0, 0)

        return canvas.toDataURL()
    }

    togglePause() {
        if (this.isRunning) {
            const pause = document.getElementById('nav-pause')!
            const pauseButton = document.getElementById('btn-pause')!

            if (!this.isPaused) {
                pause.children[0].innerHTML = `<i class="fa-solid fa-play"></i>`
                pause.children[1].textContent = 'Resume'
                pauseButton.children[0].innerHTML = `<i class="fa-solid fa-play"></i>`
                cancelAnimationFrame(this.frameNumber)
            } else {
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

        this.openFile((file) => this.startGame(file))
    }

    openFile(callback: (file: File) => void) {
        const gameInput = document.getElementById('file-game')

        if (gameInput != null) {
            gameInput.onchange = (e) => {
                const files = (e.target as HTMLInputElement)?.files

                if (files != null) {
                    const file = files[0]

                    callback(file)
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

    openAudioModal() {
        this.audioOutput.openModal()
    }

    closeAudioModal() {
        this.audioOutput.closeModal()
    }

    async startGame(gameFile: File) {
        this.isPaused = false
        this.isRunning = true

        const data = await this.readFile(gameFile)

        const gameName = gameFile.name.substring(0, gameFile.name.lastIndexOf('.'))

        const gameBytes = new Uint8Array(data)

        cancelAnimationFrame(this.frameNumber)

        this.emulator!.load_rom(gameBytes)
        this.emulator!.set_memory_card(this.memoryCardData)

        this.stateManager = new StateManager(gameName, this.rsxDb, this.emulator!)
        this.stateManager.updateStateMenuList()

        const placeholder = document.getElementById('placeholder')

        if (placeholder != null) {
            placeholder.remove()
            const canvas = document.createElement('canvas')

            canvas.id = 'psx-canvas'

            canvas.setAttribute('width', '640');
            canvas.setAttribute('height', '480')

            document.getElementById('display')!.append(canvas)

            this.emulator = new PsxWebEmulator('psx-canvas')

            this.videoOutput = new VideoOutput(canvas, this.emulator!, this.wasm!)
        } else {
            this.emulator!.reset()
        }

        this.audioOutput.setEmulator(this.emulator!)
        this.audioOutput.initAudio()

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

    toggleMute() {
        this.audioOutput.toggleMute()
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
                const samples = this.audioOutput.pushSamples()

                this.waveVisualizer.plot(samples!)

                this.joypad.handleInputAndVibration()

                this.checkSaveStatus()
            }

            this.previousTime = time - (diff % FPS_INTERVAL)
            this.frames++
            this.frameNumber = requestAnimationFrame((time) => this.runFrame(time))
        }

    }

    async checkSaveStatus() {
        const memoryCardData = this.emulator!.get_memory_bytes() as Uint8Array<ArrayBuffer>

        if (memoryCardData != null) {
            this.memoryCardData = memoryCardData
            await this.rsxDb.saveMemoryCard(this.memoryCard, this.memoryCardData)
            document.getElementById('mem-card-status')!.textContent = "Saves found"
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

        this.openFile((file) => this.swapDiscInner(file))
    }

    async swapDiscInner(gameFile: File) {
        const data = await this.readFile(gameFile)

        const bytes = new Uint8Array(data)

        this.emulator?.load_rom(bytes)
    }
}