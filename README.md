# RSX Redux

## About

This is a Playstation emulator written in Rust. It's a rewrite of the <a href="https://github.com/annethereshewent/RSX">RSX emulator</a>, with hardware rendering being the primary feature. Currently supports Metal on desktop and webGL on web, OpenGL coming soon! Web version is now available at https://rsx-redux.onrender.com/, and there is also a MacOS app available for use at https://github.com/annethereshewent/rsx-redux-macos. iOS port will eventually be available. 

## How to run

To run on your desktop locally, ensure you have a copy of the Playstation BIOS in the desktop/ directory, and use the two scripts provided in the desktop/ directory:

For the software renderer (works on any OS):

`./software.sh <path-to-rom-or-exe>`

For the hardware renderer (ARM MacOS only), do something similar:

`./hardware.sh <path-to-rom-or-exe>`

To compile the binary, use `cargo build --release` but remember to specify whether to use the hardware gpu or software renderer with `--features [hardware_gpu|software_gpu] --no-default-features`.

## Controls

Works great with a dualshock-like controller! Supports Xbox 360, dualshock, and dualsense controllers. Controls are exactly like on the Playstation.

For keyboard support (custom keyboard mappings available on the MacOS app), use:

* **Up**: W
* **Down**: S
* **Left**: A
* **Right**: D
* **Cross**: K
* **Circle**: L
* **Square**: J
* **Triangle**: I
* **L1**: U
* **R1**: O
* **L2**: 7
* **R2**: 9
* **Start**: Enter
* **Select**: Tab
* **Left stick button**: Left shift
* **Right stick button**: Right shift
* **Waveform visualizer (MacOS and web apps only)**: F4 key
* **Quick save state**: F5 key
* **Quick load state**: F7 key
* **Toggle digital mode on/off**: E Key on keyboard, touchpad button (and similar on Xbox) for controllers

## Screenshots

<img width="320" alt="Screenshot 2026-06-03 at 3 51 10 PM" src="https://github.com/user-attachments/assets/9c0ff13c-e83c-43bf-8335-fbaae66c74f6" />
<img width="320" alt="Screenshot 2026-06-05 at 11 06 05 PM" src="https://github.com/user-attachments/assets/4a4ac0ee-e553-4c28-9337-0b6d3ded2545" />
<img width="320" alt="Screenshot 2026-06-05 at 11 04 45 PM" src="https://github.com/user-attachments/assets/75693bf9-9ff2-4f9b-8071-1268a43971af" />
<img width="320" alt="Screenshot 2026-06-06 at 12 03 44 AM" src="https://github.com/user-attachments/assets/1dc263d0-24c3-444d-9037-fbd1df592191" />
<img width="320" alt="Screenshot 2026-06-06 at 12 05 06 AM" src="https://github.com/user-attachments/assets/9806ead5-c85b-4417-8864-99658e26804e" />
<img width="320" alt="Screenshot 2026-06-06 at 12 10 31 AM" src="https://github.com/user-attachments/assets/07da7869-07d4-4b8c-8dee-bed92a6c46cf" />




