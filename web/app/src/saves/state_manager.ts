import { unzlib, zlib } from "fflate";
import { PsxWebEmulator } from "../../../pkg/rsx_redux_web";
import { RsxDb } from "./rsx_db";

export class StateManager {
    db: RsxDb
    emulator: PsxWebEmulator
    gameName: string

    constructor(gameName: string, db: RsxDb, emulator: PsxWebEmulator) {
        this.db = db
        this.emulator = emulator
        this.gameName = gameName
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