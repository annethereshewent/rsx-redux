import { InitOutput, PsxWebEmulator } from "../../../pkg/rsx_redux_web"

const SAMPLE_RATE = 44100

export class AudioOutput {
    private wasm: InitOutput
    private emulator: PsxWebEmulator
    private audioContext = new AudioContext({ sampleRate: SAMPLE_RATE })
    private workletNode: AudioWorkletNode|null = null

    constructor(emulator: PsxWebEmulator, wasm: InitOutput) {
        this.emulator = emulator
        this.wasm = wasm
        this.initAudio()
    }

    async initAudio() {
        await this.audioContext.audioWorklet.addModule("audio_processing_node.js")

        this.workletNode = new AudioWorkletNode(this.audioContext, 'audio-processor', {
            numberOfOutputs: 1,
            outputChannelCount: [2]
        })
        this.workletNode.connect(this.audioContext.destination)

        await this.audioContext.resume()
    }

    pushSamples() {
        const samples = this.emulator.drain_samples()

        this.workletNode?.port.postMessage({ type: "samples", samples: samples })

        return samples
    }
}