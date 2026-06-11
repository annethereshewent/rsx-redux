import { InitOutput, PsxWebEmulator } from "../../../pkg/rsx_redux_web"

const SAMPLE_RATE = 44100

export class AudioOutput {
    private emulator: PsxWebEmulator|null = null
    private audioContext = new AudioContext({ sampleRate: SAMPLE_RATE })
    private workletNode: AudioWorkletNode|null = null
    private volumeLevel = 100
    private gainNode = this.audioContext.createGain()

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

            console.log(this.volumeLevel)

            this.gainNode.gain.value = this.volumeLevel / 100
        })
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