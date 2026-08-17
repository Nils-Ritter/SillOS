.PHONY: all build run test

all: build

build:
	cargo build

run:
	cargo build
	cargo run

test:
	cargo build
	cargo test
