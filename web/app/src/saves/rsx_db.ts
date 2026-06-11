import { DBSchema, IDBPDatabase, openDB } from "idb"

const currentVersion = 1

interface SaveState {
    data: Uint8Array
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
            data: Uint8Array
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
            upgrade(db) {
                db.createObjectStore('rsx-memory-cards', {
                    keyPath: 'name'
                })
                db.createObjectStore('rsx-save-states', {
                    keyPath: 'name'
                })
            },
        })

        this.db = db
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

        await this.db.put('rsx-memory-cards', { name: memoryCard, data })

    }
}