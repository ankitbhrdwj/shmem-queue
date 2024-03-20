[![Rust](https://github.com/ankit-iitb/shmem-queue/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/ankit-iitb/shmem-queue/actions/workflows/rust.yml)

# shmem-queue

`no_std` shared-memory queue in Rust.

## Run tests

```bash
RUST_TEST_THREADS=1 cargo test --features spsc --no-default-features
RUST_TEST_THREADS=1 cargo test --features mpsc --no-default-features
```

## Run benchmarks

```bash
taskset -c 0,1 cargo run --features spsc --no-default-features
taskset -c 0,1 cargo run --features mpsc --no-default-features
```
