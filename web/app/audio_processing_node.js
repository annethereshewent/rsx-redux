const BUFFER_SIZE = 8192
const NUM_SAMPLES = BUFFER_SIZE * 2

export class AudioProcessingNode extends AudioWorkletProcessor {
    sampleBuffer = []
    sampleIndex = 0
    constructor(context) {
        super()

        this.port.onmessage = (ev) => {
            if (ev.data.type == "samples") {
                this.sampleBuffer.push(...ev.data.samples)
            }
        }
    }

    process(inputs, outputs, parameters) {
        const left = outputs[0][0]
        const right = outputs[0][1]

        let leftIndex = 0
        let rightIndex = 0

        let isLeft = true

        while (leftIndex < left.length || rightIndex < right.length) {
            if (this.sampleIndex >= this.sampleBuffer.length) {
                this.sampleBuffer = []
                this.sampleIndex = 0
                break
            }

            let sample = this.sampleBuffer[this.sampleIndex]

            if (sample < 0) {
                sample = -sample / -32768
            } else {
                sample = sample / 32767
            }

            if (isLeft) {
                left[leftIndex] = sample
                leftIndex++
            } else {
                right[rightIndex] = sample
                rightIndex++
            }

            this.sampleIndex++

            isLeft = !isLeft
        }

        if (this.sampleBuffer.length - this.sampleIndex > NUM_SAMPLES) {
            this.sampleBuffer = this.sampleBuffer.slice(this.sampleIndex, this.sampleIndex + NUM_SAMPLES)
            this.sampleIndex = 0
        }

        return true
    }
}

registerProcessor('audio-processor', AudioProcessingNode)