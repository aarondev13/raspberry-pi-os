
<img width="4032" height="3024" alt="1" src="https://github.com/user-attachments/assets/47f9341f-947a-44cf-b149-99e0bcafba93" />
<img width="4032" height="3024" alt="2" src="https://github.com/user-attachments/assets/12ddbc76-923e-4a05-a7c8-055271d89bc6" />
<img width="3024" height="4032" alt="3" src="https://github.com/user-attachments/assets/d7e9ccab-b9d7-4dc8-afc3-a00e4fcda68e" />

# Raspberry Pi OS 1.2

### A Custom Bare-Metal Operating System for Raspberry Pi 5

Building a lightweight graphical operating system from the ground up using Rust and ARM64 Assembly.

**The mission:** Create an independent operating system that boots directly on Raspberry Pi 5 hardware without relying on Linux, X11, or Wayland as its foundation.

## Project Overview

This project aims to develop a complete bare-metal operating system for the Raspberry Pi 5 (4GB RAM).

Rather than modifying an existing Linux distribution, the goal is to build the foundational software independently, including:

- Custom ARM64 kernel
- Memory management
- Hardware communication
- Framebuffer graphics
- Custom display server and compositor
- Window management
- Graphical desktop environment

## Hardware

| Component | Specification |
|---|---|
| Board | Raspberry Pi 5 |
| RAM | 4GB |
| Processor | Broadcom BCM2712 |
| Architecture | ARM64 / AArch64 |
| I/O | RP1 southbridge |

## Technology Stack

- Rust
- ARM64 Assembly
- Bare-metal development
- Custom linker scripts
- Raspberry Pi hardware interfaces

## Development Roadmap

### Phase 1: Environment & Bootstrapping
- Rust cross-compilation environment
- Assembly entry point
- Exception vector tables
- Linker scripts
- Kernel initialization

### Phase 2: Communication & Memory
- UART debugging
- MMU implementation
- Page tables
- Heap allocation

### Phase 3: Video System
- Display hardware research
- Framebuffer initialization
- Raw pixel rendering
- Display output

### Phase 4: Graphics & Desktop
- Font rendering
- USB input support
- Display server
- Window compositor
- Custom window manager
- Desktop environment

## Development Philosophy

- Minimal resource consumption
- Fast startup
- Hardware-level control
- Independent software architecture
- Clean and maintainable code

## Current Development Status

The project is in early development.

The development environment is being established, and the initial project structure has been created.

Features will only be marked complete after implementation and testing.

## Long-Term Vision

The long-term ambition is to develop Raspberry Pi OS 1.2 into a useful, lightweight operating system and potentially build a sustainable technology business around it.

The project is open to:
- Open-source contributors
- Hardware and software mentors
- Engineering feedback
- Future commercial opportunities

## License

GNU General Public License v3.0 (GPL-3.0).

See the `LICENSE` file for details.
