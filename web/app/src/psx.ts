import init, { PsxWebEmulator, InitOutput } from "../../pkg/rsx_redux_web"
import wasmData from '../../pkg/rsx_redux_web_bg.wasm'
import { CloudService } from "./cloud_saves/cloud_service"
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
    private biosBytes = new Uint8Array([])
    private rsxDb = new RsxDb()
    private stateManager: StateManager|null = null
    private cloudService = new CloudService()

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
            if (ev.key.toLowerCase() == 'f4') {
                ev.preventDefault()
                this.waveVisualizer.toggle()
            } else if (ev.key.toLowerCase() == 'f5') {
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

    selectControllerPort(el: HTMLElement) {
        if (this.emulator != null) {
            const portStr = el.dataset.port

            if (portStr != null) {
                const port = parseInt(portStr)
                this.emulator.set_port(port)

                const unselected = portStr == '0' ? '1' : '0'

                const unselectedEl = document.querySelector(`.port-pill[data-port="${unselected}"]`)
                const selectedEl = document.querySelector(`.port-pill[data-port="${portStr}"]`)

                unselectedEl!.classList.remove('is-active')
                selectedEl!.classList.add('is-active')
            }
        }
    }

    async syncCloudCard(el: HTMLElement) {
        const slot = parseInt(el.dataset.cloudSlot ?? "null")

        if (slot != null) {
            const cardName = `memory_card${slot}`

            const loading = document.getElementById('cloud-saves-loading')

            loading!.style.display = 'block'

            const data = (await this.cloudService.getCard(cardName)).data

            loading!.style.display = 'none'

            if (data != null) {
                await this.rsxDb.saveMemoryCard(cardName, data)

                const syncCloudCard = document.querySelector(`.cloud-save-slot[data-cloud-slot="${slot}"]`)?.querySelector('.sync-cloud-card')

                if (syncCloudCard != null) {
                    (syncCloudCard as HTMLElement).innerHTML = '<i class="fa-solid fa-check"></i>'

                    setTimeout(() => {
                        (syncCloudCard as HTMLElement).innerHTML = '<i class="fa-solid fa-arrows-rotate"></i>'
                    }, 750)
                }
            }
        }
    }

    async deleteCloudCard(el: HTMLElement) {
        if (confirm("Are you sure you want to delete this memory card?")) {
            const slot = parseInt(el.dataset.cloudSlot ?? "null")

            if (slot != null) {
                const cardName = `memory_card${slot}`

                const loading = document.getElementById('cloud-saves-loading')

                loading!.style.display = 'block'

                if (await this.cloudService.deleteCard(cardName)) {
                    this.cloudService.removeModalSlot(slot)
                }

                loading!.style.display = 'none'
            }
        }
    }

    async uploadAllLocalCards() {
        const cards = await this.rsxDb.getMemoryCards()

        const loading = document.getElementById('cloud-saves-loading')
        loading!.style.display = 'block'
        const promises = []
        for (const card of cards) {
            promises.push(this.cloudService.uploadCard(card.name, card.data))
        }

        await Promise.all(promises)

        loading!.style.display = 'none'

        const timestamp = Math.floor(Date.now() / 1000)

        for (const card of cards) {
            const slot = parseInt(card.name.replace('memory_card', ''))

            this.cloudService.updateModalSlot(slot, timestamp)
        }
    }

    getCardName(el: HTMLElement) {
        const slot = parseInt(el.dataset.cloudSlot ?? "null")

        if (slot != null) {
            return `memory_card${slot}`
        }

        return null
    }

    async downloadCloudCard(el: HTMLElement) {
        const cardName = this.getCardName(el)

        if (cardName != null) {
            const loading = document.getElementById('cloud-saves-loading')

            loading!.style.display = 'block'

            const data = (await this.cloudService.getCard(cardName)).data

            loading!.style.display = 'none'

            if (data != null) {
                this.generateFile(data, cardName)
            }
        }
    }

    generateFile(data: Uint8Array, cardName: string) {
        const blob = new Blob([data.buffer as ArrayBuffer], {
            type: "application/octet-stream"
        })

        const objectUrl = URL.createObjectURL(blob)

        const a = document.createElement('a')

        a.href = objectUrl
        a.download = `${cardName}.mcd`
        document.body.append(a)
        a.style.display = "none"

        a.click()
        a.remove()

        setTimeout(() => URL.revokeObjectURL(objectUrl), 1000)
    }

    async replaceCloudCard(el: HTMLElement) {
        const slot = parseInt(el.dataset.cloudSlot ?? "null")

        if (slot != null) {
            const memoryCardName = `memory_card${slot}`

            this.openFile('cloud-saves-upload', async (file) => {
                const bytes = new Uint8Array(await this.readFile(file))

                const loading = document.getElementById('cloud-saves-loading')

                loading!.style.display = 'block'

                await this.cloudService.uploadCard(memoryCardName, bytes)

                loading!.style.display = 'none'

                const timestamp = Math.floor(Date.now() / 1000)

                this.cloudService.updateModalSlot(slot, timestamp)
            })
        }
    }

    checkOauth() {
        this.cloudService.checkAuthentication()
    }

    cloudSignIn() {
        this.cloudService.signIn()
        window.addEventListener("message", (e) => {
            if (e.data == "authFinished" && e.origin == location.origin) {
                this.cloudService.signInUser()
                this.loadMemoryCard()
            }
        }, { once: true })
    }

    cloudSignOut() {
        this.cloudService.signOut()
    }

    openCloudSaves() {
        this.cloudService.openCloudSavesModal()
    }

    closeCloudSavesModal() {
        this.cloudService.closeModal()
    }

    async loadMemoryCard() {
        const data = this.cloudService.loggedIn ? (await this.cloudService.getCard(this.memoryCard)).data : await this.rsxDb.getMemoryCard(this.memoryCard)

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
        return this.videoOutput?.getImageUrl() ?? ""
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

            this.biosBytes = biosBytes

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
        if (this.gameReady) {
            return
        }

        this.openFile('file-bios', (file) => this.handleBiosFile(file))
    }

    loadGame() {
        if (!this.biosReady) {
            return
        }

        this.openFile('file-game', (file) => this.startGame(file))
    }

    openFile(input: string, callback: (file: File) => void) {
        const gameInput = document.getElementById(input)

        if (gameInput != null) {
            gameInput.onchange = (e) => {
                const files = (e.target as HTMLInputElement)?.files

                if (files != null) {
                    const file = files[0]

                    callback(file)
                }

                (e.target as HTMLInputElement).value = ''
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

    async startGame(file: File) {
        this.isPaused = false
        this.isRunning = true

        const data = await this.readFile(file)

        const gameName = file.name.substring(0, file.name.lastIndexOf('.'))

        const binaryBytes = new Uint8Array(data)

        cancelAnimationFrame(this.frameNumber)

        const placeholder = document.getElementById('placeholder')

        if (placeholder != null) {
            placeholder.remove()
            const canvas = document.createElement('canvas')

            canvas.id = 'psx-canvas'

            canvas.setAttribute('width', '640');
            canvas.setAttribute('height', '480')

            document.getElementById('display')!.append(canvas)

            this.emulator = new PsxWebEmulator('psx-canvas')
            this.emulator.load_bios(this.biosBytes)

            // clear the bios bytes so they don't take up additional space in memory. we just need the property
            // to load the bios after a game has been chosen
            this.biosBytes = new Uint8Array([])

            this.videoOutput = new VideoOutput(canvas, this.emulator!, this.wasm!)
        } else {
            this.emulator!.reset()
        }

        this.stateManager = new StateManager(gameName, this.rsxDb, this.emulator!)
        this.stateManager.updateStateMenuList()

        if (/\.exe$/.test(file.name)) {
            this.emulator!.set_exe(binaryBytes)
        } else {
            this.emulator!.load_rom(binaryBytes)
            this.emulator!.set_exe(null)
        }

        // Even though we load the memory card on page load, this might be the "wrong" one due to a race condition where:
        // the auth token expired, it's currently signing in, and in between that, it loads the local memory card instead.
        // so we want to make sure that the emulator is using the correct memory card by loading it on game-load as well.
        await this.loadMemoryCard()

        this.audioOutput.setEmulator(this.emulator!)
        this.audioOutput.initAudio()

        this.paused = false

        document.getElementById('status-dot')!.classList.add('is-active')
        document.getElementById('status-text')!.innerText = 'Game running'

        this.joypad.updateButtonMap()
        this.joypad.addKeyboardControllerListeners()
        this.enableSwapDiscDisableBios()

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

    enableSwapDiscDisableBios() {
        this.gameReady = true
        document.getElementById('btn-swap-disc')?.removeAttribute('disabled')
        document.getElementById('btn-load-bios')?.setAttribute('disabled', 'true')
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
            if (this.cloudService.loggedIn) {
                await this.cloudService.uploadCard(this.memoryCard, this.memoryCardData)
            } else {
                await this.rsxDb.saveMemoryCard(this.memoryCard, this.memoryCardData)
            }
            document.getElementById('mem-card-status')!.textContent = "Saves found"
        }
    }

    updateFps() {
        document.getElementById('status-fps')!.innerText = `${this.fps} FPS`
    }

    async handleBiosFile(biosFile: File) {
        const dataArrayBuffer = await this.readFile(biosFile)

        const biosBytes = new Uint8Array(dataArrayBuffer)

        this.biosBytes = biosBytes

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

        this.emulator!.open_shell()

        this.openFile('file-disc', (file) => this.swapDiscInner(file))
    }

    async swapDiscInner(gameFile: File) {
        const data = await this.readFile(gameFile)

        const bytes = new Uint8Array(data)

        this.emulator?.close_shell(bytes)
    }
}