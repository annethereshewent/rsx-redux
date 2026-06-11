import { unzlib, zlib } from "fflate";
import { PsxWebEmulator } from "../../../pkg/rsx_redux_web";
import { RsxDb } from "./rsx_db";

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

    async createSaveState(index: number, imageUrl: string) {

        const data = this.emulator.save_state()

        return new Promise((resolve, _reject) => {
            zlib(data, { level: 2 }, async (err, compressed) => {
                if (err) {
                    console.log(err)
                    resolve(null)
                } else {
                    const entry = await this.db.saveState(this.gameName, index, compressed, imageUrl)
                    resolve(entry)
                }
            })
        })
    }
    async loadSaveState(index: number): Promise<Uint8Array|null> {
        const compressed = await this.db.loadState(this.gameName, index)

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
}