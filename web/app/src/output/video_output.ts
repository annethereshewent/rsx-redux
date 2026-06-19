import { InitOutput, PsxWebEmulator } from "../../../pkg/rsx_redux_web"

const SCREEN_WIDTH = 640
const SCREEN_HEIGHT = 480

export class VideoOutput {
    private canvas: HTMLCanvasElement
    private wasm: InitOutput
    private emulator: PsxWebEmulator
    private context: CanvasRenderingContext2D

    constructor(canvas: HTMLCanvasElement, emulator: PsxWebEmulator, wasm: InitOutput) {
        this.canvas = canvas
        this.context = canvas.getContext("2d")!
        this.emulator = emulator
        this.wasm = wasm
    }

    // updateCanvas() {
    //     const emu = this.emulator
    //     const memory = new Uint8Array(this.wasm.memory.buffer, emu.get_framebuffer(), emu.get_framebuffer_size())
    //     const [width, height] = emu.get_dimensions()

    //     const imageData = this.context.getImageData(0, 0, width, height)

    //     this.canvas.setAttribute('width', `${width}`)
    //     this.canvas.setAttribute('height', `${height}`)

    //     for (let y = 0; y < width; y++) {
    //         for (let x = 0; x < height; x++) {
    //             const index = x * 3 + y * height * 3
    //             const canvasIndex = x * 4 + y * height * 4

    //             imageData.data[canvasIndex] = memory[index]
    //             imageData.data[canvasIndex + 1] = memory[index + 1]
    //             imageData.data[canvasIndex + 2] = memory[index + 2]
    //             imageData.data[canvasIndex + 3] = 255
    //         }
    //     }

    //     this.context.putImageData(imageData, 0, 0)
    // }

    getImageUrl() {
        return this.canvas?.toDataURL() ?? ""
    }

    updateCanvas() {
        const [width, height] = this.emulator.get_dimensions();

        console.log(`width = ${width} height = ${height}`);

        this.canvas.setAttribute('width', `${width}`)
        this.canvas.setAttribute('height', `${height}`)
    }
}