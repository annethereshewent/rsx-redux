// ════════════════════════════════════════════
// Stacked stereo waveform visualizer with min/max downsampling
// Left channel = top lane (blue), Right channel = bottom lane (pink)
// Assumes interleaved i16 stereo samples (L, R, L, R...)
// ════════════════════════════════════════════

export class WaveVisualizer {
    private canvas: HTMLCanvasElement
    private ctx: CanvasRenderingContext2D
    private enabled = false

    constructor() {
        this.canvas = document.getElementById('waveform-canvas') as HTMLCanvasElement
        this.ctx = this.canvas.getContext('2d')!
        this.resize()
        window.addEventListener('resize', () => this.resize())
    }

    private resize() {
        const dpr = window.devicePixelRatio || 1
        const rect = this.canvas.getBoundingClientRect()
        this.canvas.width = rect.width * dpr
        this.canvas.height = rect.height * dpr
        this.ctx.setTransform(1, 0, 0, 1, 0, 0) // reset before scaling
        this.ctx.scale(dpr, dpr)
    }

    toggle(): boolean {
        this.enabled = !this.enabled
        const wrapper = document.getElementById('waveform-wrapper')!
        wrapper.style.display = this.enabled ? 'block' : 'none'
        if (this.enabled) this.resize()
        return this.enabled
    }

    // samples: interleaved i16 stereo (L, R, L, R...)
    plot(samples: Int16Array) {
        if (!this.enabled) return

        const rect = this.canvas.getBoundingClientRect()
        const w = rect.width
        const h = rect.height

        this.ctx.clearRect(0, 0, w, h)

        // two stacked lanes: left on top, right on bottom
        const laneHeight = h / 2
        const leftCenter = laneHeight / 2
        const rightCenter = laneHeight + laneHeight / 2
        const amplitude = laneHeight * 0.45 // leave a little headroom per lane

        // lane divider
        this.ctx.strokeStyle = 'rgba(42, 58, 92, 0.8)'
        this.ctx.lineWidth = 1
        this.ctx.beginPath()
        this.ctx.moveTo(0, laneHeight)
        this.ctx.lineTo(w, laneHeight)
        this.ctx.stroke()

        // per-lane center lines
        this.ctx.strokeStyle = 'rgba(42, 58, 92, 0.4)'
        this.ctx.beginPath()
        this.ctx.moveTo(0, leftCenter)
        this.ctx.lineTo(w, leftCenter)
        this.ctx.moveTo(0, rightCenter)
        this.ctx.lineTo(w, rightCenter)
        this.ctx.stroke()

        this.drawChannel(samples, 0, w, leftCenter, amplitude, '#6c8cbf')    // left, blue
        this.drawChannel(samples, 1, w, rightCenter, amplitude, '#c2608e') // right, pink
    }

    private drawChannel(
        samples: Int16Array,
        offset: number,        // 0 = left, 1 = right
        w: number,
        centerY: number,
        amplitude: number,
        color: string
    ) {
        const frameCount = Math.floor(samples.length / 2)
        if (frameCount === 0) return

        const pixelWidth = Math.max(1, Math.floor(w))

        this.ctx.strokeStyle = color
        this.ctx.lineWidth = 1
        this.ctx.beginPath()

        if (frameCount <= pixelWidth) {
            // fewer samples than pixels — just draw a normal line
            const step = w / frameCount
            for (let i = 0; i < frameCount; i++) {
                const norm = this.normalize(samples[i * 2 + offset])
                const x = i * step
                const y = centerY - norm * amplitude
                if (i === 0) this.ctx.moveTo(x, y)
                else this.ctx.lineTo(x, y)
            }
            this.ctx.stroke()
            return
        }

        // more samples than pixels — min/max downsample per pixel column.
        // for each x pixel we find the min and max sample in that bucket and
        // draw a vertical line between them, which preserves the visual
        // "thickness" of loud passages instead of aliasing them away.
        const samplesPerPixel = frameCount / pixelWidth

        for (let x = 0; x < pixelWidth; x++) {
            const start = Math.floor(x * samplesPerPixel)
            const end = Math.min(frameCount, Math.floor((x + 1) * samplesPerPixel))

            let min = 1.0
            let max = -1.0
            for (let i = start; i < end; i++) {
                const norm = this.normalize(samples[i * 2 + offset])
                if (norm < min) min = norm
                if (norm > max) max = norm
            }

            const yMax = centerY - max * amplitude
            const yMin = centerY - min * amplitude
            this.ctx.moveTo(x + 0.5, yMax)
            this.ctx.lineTo(x + 0.5, yMin)
        }

        this.ctx.stroke()
    }

    private normalize(raw: number): number {
        return raw < 0 ? raw / 32768 : raw / 32767
    }
}