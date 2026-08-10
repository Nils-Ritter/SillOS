# SillOS

SillOS is a small hobby operating system for x86_64, written in Rust.

The project is intended for learning and experimentation with operating-system development, including kernel initialization, memory management, hardware access, interrupts, and bare-metal Rust programming.

> **Status:** Early development

## Features

- x86_64 kernel
- Written in Rust
- `no_std` bare-metal environment
- Bootable using the Rust `bootloader` and `bootimage` tools
- Serial output for debugging
- Kernel tests running inside QEMU

## Requirements

- Linux, macOS, or Windows with WSL
- Rust nightly toolchain
- `rust-src` component
- `llvm-tools-preview` component
- QEMU
- `bootimage`

Install the Rust components and tools with:

```sh
rustup toolchain install nightly
rustup component add rust-src llvm-tools-preview --toolchain nightly
cargo install bootimage
```

On Debian or Ubuntu, install QEMU with:

```sh
sudo apt update
sudo apt install qemu-system-x86 qemu-utils
```

## Building

Clone the repository:

```sh
git clone https://github.com/Nils-Ritter/SillOS.git
cd SillOS
```

Build the kernel and create a bootable image:

```sh
cargo bootimage
```

The generated images are placed in the `target` directory.

## Running

Run the operating system in QEMU:

```sh
cargo run
```

If `cargo run` is not configured as a runner, start the generated image manually:

```sh
qemu-system-x86_64 \
  -drive format=raw,file=target/x86_64-sillos/debug/bootimage-SillOS.bin \
  -serial stdio \
  -display none
```

The exact image path may differ depending on the selected profile and target configuration.

## Testing

Run the kernel tests inside QEMU:

```sh
cargo test
```

Tests are executed using the configured `bootimage` runner. Kernel output is normally written to the serial console.

## Project Structure

```text
.
├── .cargo/
│   └── config.toml       # Cargo target and runner configuration
├── src/
│   └── main.rs           # Kernel entry point
├── Cargo.toml            # Project manifest
├── Cargo.lock            # Locked dependency versions
└── README.md
```

## Development

Format the source code:

```sh
cargo fmt
```

Check the project without running it:

```sh
cargo check
```

Run Clippy where supported:

```sh
cargo clippy
```

Because SillOS is a bare-metal project, some standard Rust tooling and libraries are not available inside the kernel.

## Continuous Integration

GitHub Actions can build and test SillOS using Rust nightly, QEMU, and `bootimage`.

The CI workflow should install:

- Rust nightly
- `rust-src`
- `llvm-tools-preview`
- QEMU
- `bootimage`

## Roadmap

- [ ] Improve serial logging
- [x] Add interrupt handling
- [x] Add memory management
- [x] Add a keyboard driver
- [ ] Add a basic shell
- [x] Add a simple heap allocator
- [ ] Improve automated testing
- [ ] Support additional hardware

## License

This project is licensed under the terms of the license included in this repository.
