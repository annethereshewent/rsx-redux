import { PsxWebEmulator } from "../../../pkg/rsx_redux_web"

const SAMPLE_RATE = 44100

export class AudioOutput {
    private emulator: PsxWebEmulator|null = null
    private audioContext = new AudioContext({ sampleRate: SAMPLE_RATE })
    private workletNode: AudioWorkletNode|null = null
    private volumeLevel = 100
    private gainNode = this.audioContext.createGain()
    private isMuted = false

    private audioClickListener = (event: Event) => {
        const modal = document.getElementById('audio-modal')
        const modalBox = modal?.children[0]
        if (!modalBox?.contains((event.target as HTMLElement)!) && modal?.classList.contains('is-active')) {
            this.closeModal()
        }
    }

    constructor() {
        document.getElementById('volume-slider')!.addEventListener('change', (ev) => {
            this.volumeLevel = parseInt((ev.target as HTMLInputElement).value)
            document.getElementById('volume-value')!.textContent = `${this.volumeLevel}%`

            this.gainNode.gain.value = this.volumeLevel / 100

            localStorage.setItem('rsx-current-volume', this.volumeLevel.toString())
        })
    }

    toggleMute() {
        this.isMuted = !this.isMuted
        this.setMute()

        localStorage.setItem('rsx-is-muted', JSON.stringify(this.isMuted))
    }

    setMute() {
        const muteIcon = document.getElementById('mute-icon')!
        const slider = document.getElementById('volume-slider') as HTMLInputElement
        const volumeValue = document.getElementById('volume-value')!
        if (!this.isMuted) {
            this.gainNode.gain.value = this.volumeLevel / 100
            muteIcon.classList.remove('fa-volume-xmark')
            muteIcon.classList.add('fa-volume-high')

            slider.value = `${this.volumeLevel}`
            volumeValue.textContent = `${this.volumeLevel}%`
        } else {
            this.gainNode.gain.value = 0
            muteIcon?.classList.remove('fa-volume-high')
            muteIcon?.classList.add('fa-volume-xmark')

            slider.value = '0'
            volumeValue.textContent = '0%'
        }
    }

    closeModal() {
        const modal = document.getElementById('audio-modal')
        modal?.classList.remove('is-active')
        document.removeEventListener('click', this.audioClickListener)
    }

    openModal() {
        document.removeEventListener('click', this.audioClickListener)
        document.getElementById('audio-modal')?.classList.add('is-active')

        document.addEventListener('click', this.audioClickListener)
    }

    async initAudio() {
        await this.audioContext.audioWorklet.addModule("audio_processing_node.js")

        this.workletNode = new AudioWorkletNode(this.audioContext, 'audio-processor', {
            numberOfOutputs: 1,
            outputChannelCount: [2]
        })

        this.workletNode.connect(this.gainNode)
        this.gainNode.connect(this.audioContext.destination)

        this.gainNode.gain.value = this.volumeLevel / 100

        const savedVolume = localStorage.getItem('rsx-current-volume')

        if (savedVolume != null) {
            this.volumeLevel = parseInt(savedVolume)
            this.gainNode.gain.value = this.volumeLevel / 100
            // javascript is being stupid so i need to add a semicolon here. otherwise it thinks i'm trying
            // to call a function on volumeSlider on the next line
            const volumeSlider = document.getElementById('volume-slider');

            (volumeSlider as HTMLInputElement).value = savedVolume
        }

        const isMuted = localStorage.getItem('rsx-is-muted')

        if (isMuted != null) {
            this.isMuted = JSON.parse(isMuted)
            this.setMute()
        }


        await this.audioContext.resume()
    }

    setEmulator(emulator: PsxWebEmulator) {
        this.emulator = emulator
    }

    pushSamples() {
        const samples = this.emulator?.drain_samples()

        this.workletNode?.port.postMessage({ type: "samples", samples: samples })

        return samples
    }
}