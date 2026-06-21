import { DBSchema, IDBPDatabase, openDB } from "idb"
import moment from "moment"

const currentVersion = 1

export interface SaveState {
    data: Uint8Array
    imageUrl: string
    timestamp: number
}

interface GameEntry {
    gameName: string
    saveStates: SaveState[]
}

interface RsxDB extends DBSchema {
    'rsx-save-states': {
        key: string,
        value: {
            gameName: string,
            saveStates: SaveState[]
        }
    },
    'rsx-memory-cards': {
        key: string,
        value: {
            name: string,
            data: Uint8Array,
            lastModified?: number
        }
    }
}

export class RsxDb {
    private db: IDBPDatabase<RsxDB>|null = null

    constructor() {
        this.loadDb()
    }

    async loadDb() {
        const db = await openDB<RsxDB>('rsx-db', currentVersion, {
            async upgrade(db) {
                db.createObjectStore('rsx-memory-cards', {
                    keyPath: 'name'
                })

                db.createObjectStore('rsx-save-states', {
                    keyPath: 'gameName'
                })
            },
        })

        this.db = db
    }

    async saveState(gameName: string, index: number, data: Uint8Array, imageUrl: string): Promise<SaveState|null> {
        if (this.db == null) {
            this.db = await openDB('rsx-db', currentVersion)
        }

        const entry = await this.db.get('rsx-save-states', gameName)

        if (entry != null) {
            entry.saveStates[index] = {
                data,
                imageUrl,
                timestamp: moment().unix()
            }

            this.db.put('rsx-save-states', entry)

            return entry.saveStates[index]
        } else {
            const entry: GameEntry = {
                gameName,
                saveStates: []
            }

            entry.saveStates[index] = {
                data,
                imageUrl,
                timestamp: moment().unix()
            }

            this.db.put('rsx-save-states', entry)

            return entry.saveStates[index]
        }
    }

    async deleteState(gameName: string, index: number) {
        if (this.db == null) {
            this.db = await openDB('rsx-db', currentVersion)
        }
        const entry = await this.db.get('rsx-save-states', gameName)

        if (entry != null) {
            delete(entry.saveStates[index])

            this.db.put('rsx-save-states', entry)
        }
    }

    async getSaveStates(gameName: string) {
        if (this.db == null) {
            this.db = await openDB('rsx-db', currentVersion)
        }

        const entry = await this.db.get('rsx-save-states', gameName)

        return entry?.saveStates
    }

    async loadState(gameName: string, index: number) {
        if (this.db == null) {
            this.db = await openDB('rsx-db', currentVersion)
        }

        const entry = await this.db.get('rsx-save-states', gameName)

        return entry?.saveStates[index].data || new Uint8Array([])
    }

    async getMemoryCard(memoryCard: string) {
        if (this.db == null) {
            this.db = await openDB("rsx-db", currentVersion)
        }
        const card = await this.db.get('rsx-memory-cards', memoryCard)

        return card?.data
    }

    async saveMemoryCard(memoryCard: string, data: Uint8Array) {
        if (this.db == null) {
            this.db = await openDB('rsx-db')
        }

        await this.db.put('rsx-memory-cards', { name: memoryCard, data, lastModified: Math.floor(Date.now() / 1000) })

    }

    async getMemoryCards() {
        if (this.db == null) {
            this.db = await openDB("rsx-db", currentVersion)
        }

        const cards = await this.db.getAll('rsx-memory-cards')

        return cards
    }
}