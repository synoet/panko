# Build and symlink to ~/.cargo/bin for local development
link:
    cargo build --release
    ln -sf {{justfile_directory()}}/target/release/panko ~/.cargo/bin/panko

# Build debug
build:
    cargo build

# Build release
release:
    cargo build --release

# Run with arguments
run *args:
    cargo run -- {{args}}
