[![Rust](https://github.com/ankit-iitb/shmem-queue/actions/workflows/rust.yml/badge.svg?branch=main)](https://github.com/ankit-iitb/shmem-queue/actions/workflows/rust.yml)

# shmem-queue

`no_std` shared-memory queue in Rust.

## Run tests

```bash
cargo test --features spsc --no-default-features
cargo test --features mpmc --no-default-features
```

## Run benchmarks

```bash
cargo bench --features spsc --no-default-features
cargo bench --features mpmc --no-default-features
```
