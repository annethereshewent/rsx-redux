import { unzlib, zlib } from "fflate";
import { PsxWebEmulator } from "../../../pkg/rsx_redux_web";
import { RsxDb, SaveState } from "./rsx_db";
import moment from "moment";

export class StateManager {
    private db: RsxDb
    private emulator: PsxWebEmulator
    private gameName: string
    private saveStateModalListener = (event: Event) => {
        const modal = document.getElementById('save-states-modal')
        const modalBox = modal?.children[0]
        if (!modalBox?.contains((event.target as HTMLElement)!) && modal?.classList.contains('is-active')) {
            this.closeModal()
        }
    }

    constructor(gameName: string, db: RsxDb, emulator: PsxWebEmulator) {
        this.db = db
        this.emulator = emulator
        this.gameName = gameName
    }

    openSaveStatesModal() {
        this.updateStateModal()

        document.removeEventListener('click', this.saveStateModalListener)
        document.getElementById('save-states-modal')?.classList.add('is-active')

        document.addEventListener('click', this.saveStateModalListener)

        const event = new CustomEvent('savestatemodalchange')

        document.dispatchEvent(event)
    }


    closeModal() {
        const modal = document.getElementById('save-states-modal')
        modal?.classList.remove('is-active')
        document.removeEventListener('click', this.saveStateModalListener)

        const event = new CustomEvent('savestatemodalchange')

        document.dispatchEvent(event)
    }

    async createSaveState(index: number, imageUrl: string): Promise<SaveState|null> {

        const data = this.emulator.save_state()

        return new Promise((resolve, _reject) => {
            zlib(data, { level: 2 }, async (err, compressed) => {
                if (err) {
                    console.log(err)
                    resolve(null)
                } else {
                    const state = await this.db.saveState(this.gameName, index, compressed, imageUrl)

                    if (state != null) {
                        this.updateStateModalEntry(index, state)
                    }

                    resolve(state)
                }
            })
        })
    }
    async loadSaveState(index: number): Promise<Uint8Array|null> {
        const compressed = await this.db.loadState(this.gameName, index)

        this.closeModal()

        if (compressed != null) {
            return await this.decompress(compressed)
        }

        return null
    }

    async decompress(compressed: Uint8Array): Promise<Uint8Array|null> {
        return new Promise((resolve, reject) => {
            unzlib(compressed, (err, data) => {
                if (err) {
                    console.log(err)
                    resolve(null)
                } else    {
                    resolve(data)
                }
            })
        })
    }

    updateStateModalEntry(index: number, state: SaveState) {
        const indexName = index == 0 ? 'quick' : `${index}`
        const slot = document.querySelector(`.save-slot[data-slot="${indexName}"]`)

        if (slot != null) {
            const saveSlotThumb = slot.querySelector('.save-slot-thumb')
            const screenshot = slot.querySelector('.save-slot-screenshot') as HTMLImageElement|null
            const gameName = slot.querySelector('.save-slot-game')
            const saveDate = slot.querySelector('.save-slot-date')

            const actions = slot.querySelector('.save-slot-actions')

            if (screenshot != null) {
                screenshot.src = state.imageUrl
            } else {
                const imageEl = document.createElement('img')
                imageEl.classList.add('save-slot-screenshot')
                imageEl.src = state.imageUrl

                saveSlotThumb!.innerHTML = ''
                saveSlotThumb!.append(imageEl)
            }

            gameName!.textContent = this.gameName
            gameName!.classList.remove('is-empty-text')
            saveDate!.textContent = moment.unix(state.timestamp).format('lll')

            for (const child of actions!.children) {
                (child as HTMLElement).style.display = 'block'
            }
        }

    }

    async updateStateModal() {
        const saveStates = await this.db.getSaveStates(this.gameName)

        if (saveStates != null) {
            for (let i = 0; i <= 5; i++) {
                if (saveStates[i] != null) {
                    this.updateStateModalEntry(i, saveStates[i])
                }
            }
        }
    }
}