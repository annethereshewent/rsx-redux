import moment from "moment"

const BASE_URL = "https://accounts.google.com/o/oauth2/v2/auth"
const CLIENT_ID = "353451169812-j73f39lk2j30jkvtdshub7l7r08nj0iv.apps.googleusercontent.com"

export interface FileEntry {
    cardName: string,
    data?: Uint8Array,
    lastModified?: number
  }

export class CloudService {
    loggedIn = false
    private accessToken = ""
    private rsxFolderId: string|null = null
    private modalClickListener = (event: Event) => {
        const modal = document.getElementById('cloud-saves-modal')
        const modalBox = modal?.children[0]
        if (!modalBox?.contains((event.target as HTMLElement)!) && modal?.classList.contains('is-active')) {
            this.closeModal()
        }
    }

    constructor() {
        const queryParams = new URL(document.location.toString()).searchParams

        if (queryParams.has("oauth")) {
            const params = this.getLoginParams()

            location.href = `${BASE_URL}?${params.toString()}`
        }

        const accessToken = localStorage.getItem('rsx_access_token')
        const expiresIn = parseInt(localStorage.getItem('rsx_access_expires') || 'null')
        const rsxFolderId = localStorage.getItem("rsx_folder_id")

        if (rsxFolderId != null) {
            this.rsxFolderId = rsxFolderId
        }

        if (accessToken == null) {
            this.loggedIn = false

            this.signOutUser()

            if (localStorage.getItem("rsx_user_email") != null) {
                this.silentSignIn()
            }
        } else if (expiresIn != null && (Date.now() < expiresIn)) {
            this.accessToken = accessToken

            this.signInUser()
        } else {
            localStorage.removeItem("rsx_access_token")
            localStorage.removeItem("rsx_access_expires")
            localStorage.removeItem("rsx_folder_id")

            this.silentSignIn()
        }

        window.addEventListener("message", (e) => {
            if (e.data == "authFinished") {
                this.signInUser()
            }
        })
    }

    closeModal() {
        const modal = document.getElementById('cloud-saves-modal')
        modal?.classList.remove('is-active')
        document.removeEventListener('click', this.modalClickListener)
    }

    async openCloudSavesModal() {
        if (!this.loggedIn) {
            return
        }
        const modal = document.getElementById('cloud-saves-modal')

        document.removeEventListener('click', this.modalClickListener)

        modal?.classList.add('is-active')

        document.addEventListener('click', this.modalClickListener)

        const loading = document.getElementById('cloud-saves-loading')

        if (loading != null) {
            loading.style.display = 'block'

            const files = await this.getCards()

            const quota = document.getElementById('cloud-saves-quota')

            if (quota != null) {
                quota.textContent = `${files.length} / 5 memory cards used`
            }

            loading.style.display = 'none'

            for (const file of files) {
                const slot = parseInt(file.cardName.replace('memory_card', '').replace('.mcd', ''))
                this.updateModalSlot(slot, file.lastModified ?? Date.now())
            }
        }


    }

    signOut() {
        localStorage.removeItem("rsx_access_token")
        localStorage.removeItem("rsx_access_expires")
        localStorage.removeItem("rsx_user_email")
        localStorage.removeItem("rsx_folder_id")

        this.loggedIn = false
        this.accessToken = ""

        this.signOutUser()
    }

    signOutUser() {
        const signIn = document.getElementById("cloud-sign-in")

        if (signIn != null) {
            signIn.style.display = "block"
            document.getElementById("cloud-sign-out")?.classList.remove('is-active')
        }
    }

    signIn() {
        window.open(`${location.href}?oauth=true`, "popup", "popup=true,width=650,height=650,resizable=true")
    }


    async getCardInfo(cardName: string, searchRoot: boolean = false) {
        await this.createRsxSavesFolder()

        const query = searchRoot ? `name = "${cardName}.mcd"` : `name = "${cardName}.mcd" and parents in "${this.rsxFolderId}"`

        const params = new URLSearchParams({
            q: query,
            fields: "files/id,files/parents,files/name,files/modifiedTime"
        })

        const url = `https://www.googleapis.com/drive/v3/files?${params.toString()}`

        return await this.cloudRequest(() => fetch(url, {
            headers: {
                Authorization: `Bearer ${this.accessToken}`
            }
        }))
    }

    async getCard(cardName: string): Promise<FileEntry> {
        const json = await this.getCardInfo(cardName)

        if (json != null && json.files != null) {
            const file = json.files[0]

            if (file != null) {

                // retrieve the file data from the cloud
                const url = `https://www.googleapis.com/drive/v3/files/${file.id}?alt=media`

                const body = await this.cloudRequest(() => fetch(url, {
                    headers: {
                        Authorization: `Bearer ${this.accessToken}`
                    }
                }), true)

                const returnVal = {
                    cardName,
                    lastModified: moment(file.modifiedTime).unix(),
                    data: new Uint8Array((body as ArrayBuffer)),
                }

                return returnVal
            }

        }

        return {
            cardName,
            data: undefined
        }
    }

    async deleteSave(cardName: string): Promise<boolean> {
        const json = await this.getCardInfo(cardName)

        if (json != null && json.files != null) {
            const url = `https://www.googleapis.com/drive/v3/files/${json.files[0].id}`

            await this.cloudRequest(() => fetch(url, {
                headers: {
                    Authorization: `Bearer ${this.accessToken}`
                },
                method: "DELETE"
            }))

            return true
        }

        return false
    }

    async getCards(): Promise<FileEntry[]> {
        await this.createRsxSavesFolder()

        const params = new URLSearchParams({
            q: `parents in "${this.rsxFolderId}"`,
            fields: "files/modifiedTime, files/name"
        })
        const url = `https://www.googleapis.com/drive/v3/files?${params.toString()}`

        const json = await this.cloudRequest(() => fetch(url, {
            headers: {
                Authorization: `Bearer ${this.accessToken}`
            }
        }))

        const saveEntries: FileEntry[] = []
        if (json != null && json.files != null) {
            for (const file of json.files) {
                saveEntries.push({
                    cardName: file.name,
                    lastModified: moment(file.modifiedTime).unix()
                })
            }
        }

        return saveEntries
    }

    updateModalSlot(slot: number, timestamp: number) {
        const cloudSlotElement = document.querySelector(`.cloud-save-slot[data-cloud-slot="${slot}"]`)

        if (cloudSlotElement != null) {
            cloudSlotElement.classList.remove('is-empty')
            cloudSlotElement.innerHTML = `
                <div class="cloud-save-icon">
                    <i class="fa-solid fa-sd-card"></i>
                </div>
                <div class="cloud-save-info">
                    <div class="cloud-save-header">
                        <span class="cloud-save-label">Card ${slot}</span>
                    </div>
                    <p class="cloud-save-date">Last updated ${moment.unix(timestamp).format('lll')}</p>
                </div>
                <div class="cloud-save-actions">
                    <button class="button is-psx is-small" data-action="syncCloudCard" data-cloud-slot="${slot}" title="Sync to local" aria-label="Sync Card ${slot} to local">
                        <i class="fa-solid fa-arrows-rotate"></i>
                    </button>
                    <button class="button is-psx is-small" data-action="downloadCloudCard" data-cloud-slot="${slot}" title="Download as file" aria-label="Download Card ${slot} as a file">
                        <i class="fa-solid fa-file-export"></i>
                    </button>
                    <button class="button is-psx is-small" data-action="replaceCloudCard" data-cloud-slot="${slot}" title="Upload a file to replace this slot" aria-label="Replace Card ${slot} from file">
                        <i class="fa-solid fa-file-arrow-up"></i>
                    </button>
                    <button class="button is-psx is-small is-delete" data-action="deleteCloudCard" data-cloud-slot="${slot}" title="Delete from cloud" aria-label="Delete Card ${slot} from cloud">
                        <i class="fa-solid fa-trash-can"></i>
                    </button>
                </div>
            `
        }
    }

    async uploadCard(cardName: string, bytes: Uint8Array) {
        const json = await this.getCardInfo(cardName)

        // this is a hack to get it to change the underlying array buffer
        // (so it doesn't save a bunch of junk from memory unrelated to save)

        const payload = new Uint8Array(Array.from(bytes))

        const buffer = payload.buffer

        let resultFile: any
        if (json != null && json.files != null) {
            const file = json.files[0]

            if (file != null) {
                const url = `https://www.googleapis.com/upload/drive/v3/files/${file.id}?uploadType=media`
                await this.cloudRequest(() => fetch(url, {
                    method: "PATCH",
                    headers: {
                        Authorization: `Bearer ${this.accessToken}`,
                        "Content-Type": "application/octet-stream",
                        "Content-Length": `${payload.length}`
                    },
                    body: buffer
                }))
                // there's no need for renaming the file since it's already been uploaded
                return
            } else {
                const url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=media&fields=id,name,parents"
                resultFile = await this.cloudRequest(() => fetch(url, {
                    method: "POST",
                    headers: {
                        Authorization: `Bearer ${this.accessToken}`,
                        "Content-Type": "application/octet-stream",
                        "Content-Length": `${payload.length}`
                    },
                    body: buffer
                }))
            }
        }

        if (resultFile != null) {
            let fileName = `${cardName}.mcd`

            const params = new URLSearchParams({
                uploadType: "media",
                addParents: this.rsxFolderId || "",
                removeParents: resultFile.parents.join(",")
            })

            const url = `https://www.googleapis.com/drive/v3/files/${resultFile.id}?${params.toString()}`

            await this.cloudRequest(() => fetch(url, {
                method: "PATCH",
                headers: {
                    Authorization: `Bearer ${this.accessToken}`,
                    "Content-Type": "application/octet-stream"
                },
                body: JSON.stringify({
                    name: fileName,
                    mimeType: "application/octet-stream"
                })
            }))
        }
    }

    async createRsxSavesFolder() {
        if (this.rsxFolderId == null) {
            const params = new URLSearchParams({
                q: `mimeType = "application/vnd.google-apps.folder" and name="rsx-cards"`
            })
            const url = `https://www.googleapis.com/drive/v3/files?${params.toString()}`

            const json = await this.cloudRequest(() => fetch(url, {
                headers: {
                    Authorization: `Bearer ${this.accessToken}`
                },
            }))

            if (json != null && json.files != null && json.files[0] != null) {
                this.rsxFolderId = json.files[0].id
                localStorage.setItem("rsx_folder_id", this.rsxFolderId!!)
            } else {
                // create the folder
                const url = `https://www.googleapis.com/drive/v3/files?uploadType=media`

                const json = await this.cloudRequest(() => fetch(url, {
                    method: "POST",
                    headers: {
                        Authorization: `Bearer ${this.accessToken}`,
                        "Content-Type": "application/vnd.google-apps.folder"
                    },
                    body: JSON.stringify({
                        name: "rsx-cards",
                        mimeType: "application/vnd.google-apps.folder"
                    })
                }))


                if (json != null && json.files != null && json.files[0] != null) {
                    this.rsxFolderId = json.files[0].id
                }
            }
        }
    }

    getLoginParams(noPrompt: boolean = false) {
        // since it always redirects back to the root, location.href should be fine (hopefully!)
        const params = new URLSearchParams({
            client_id: CLIENT_ID,
            redirect_uri: location.href.split('?')[0].replace(/\/$/, ''), // remove the trailing slash
            response_type: "token",
            scope: "https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/userinfo.email",
        })

        if (noPrompt) {
            const email = localStorage.getItem("rsx_user_email")

            if (email != null) {
                params.append("prompt", "none")
                params.append("login_hint", email)
            }

        }

        return params
    }

    signInUser() {
        const accessToken = localStorage.getItem('rsx_access_token')
        if (accessToken != null) {
            const signIn = document.getElementById("cloud-sign-in")
            this.loggedIn = true
            this.accessToken = accessToken
            if (signIn != null) {
                signIn.classList.remove('is-active')
                signIn.style.display = 'none'
                const signOut = document.getElementById("cloud-sign-out")

                if (signOut != null) {
                    signOut.classList.add('is-active')
                }
            }
        }


    }

    async checkAuthentication() {
        if (window.location.href.indexOf("#") != -1) {
            const tokenParams = window.location.href.split("#")[1].split("&")

            let accessToken = tokenParams.filter((param) => param.indexOf('access_token') != -1)[0]
            let expires = tokenParams.filter((param) => param.indexOf('expires_in') != -1)[0]

            if (accessToken != null) {
                accessToken = accessToken.split("=")[1]

                if (expires != null) {
                    expires = expires.split("=")[1]

                    const timestamp = parseInt(expires) * 1000 + Date.now()

                    localStorage.setItem("rsx_access_expires", timestamp.toString())
                }

                localStorage.setItem("rsx_access_token", accessToken)

                this.accessToken = accessToken
                this.signInUser()

                // finally get logged in user email
                await this.getLoggedInEmail()

                parent.postMessage("authFinished", "*")

                window.opener?.postMessage("authFinished", "*")

                window.close()
            }
        }
    }

    async getLoggedInEmail() {
        const url = "https://www.googleapis.com/oauth2/v2/userinfo"

        const json = await this.cloudRequest(() => fetch(url, {
            headers: {
                Authorization: `Bearer ${this.accessToken}`
            }
        }))

        if (json != null && json.email != null) {
            localStorage.setItem("rsx_user_email", json.email)
        }
    }

    async cloudRequest(request: () => Promise<Response>, returnBuffer: boolean = false): Promise<any> {
        return new Promise(async (resolve, reject) => {
            await this.refreshTokensIfNeeded()

            const response = await request()

            if (response.status == 200) {
                const data = returnBuffer ? await response.arrayBuffer() : response.json()

                resolve(data)
            } else if (response.status == 401) {

                this.signOut()

                const notification = document.getElementById("request-failure-notification")!

                notification.style.display = "block"

                let opacity = 1.0

                let interval = setInterval(() => {
                    opacity -= 0.05
                    notification.style.opacity = `${opacity}`

                    if (opacity <= 0) {
                        clearInterval(interval)
                    }
                }, 100)

                resolve(null)
            } else if (response.status == 404) {
                const data = await response.json()
                resolve(data)
            }
        })
    }

    silentSignIn() {
        const silentEl = document.getElementById("silent-sign-in") as HTMLIFrameElement

        if (silentEl != null && silentEl.contentWindow != null) {
            const params = this.getLoginParams(true)

            silentEl.contentWindow.window.location.href = `${BASE_URL}?${params.toString()}`
        }
    }

    private refreshTokensIfNeeded() {
        const userEmail = localStorage.getItem("rsx_user_email")

        if (userEmail == null) {
            return
        }

        return new Promise((resolve, reject) => {
            const rsxExpires = parseInt(localStorage.getItem("rsx_access_expires") || "-1")
            if (rsxExpires != null && (Date.now() >= rsxExpires || rsxExpires == -1)) {
                // refresh tokens as they're expired
                window.addEventListener("message", async (e) => {
                    if (e.data == "authFinished") {
                        this.signInUser()

                        resolve(null)
                    }
                })

                this.silentSignIn()
            } else {
                resolve(null)
            }
        })
    }
}