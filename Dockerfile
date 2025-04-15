# === Stage 1: Builder ===
# use a newer rust version that supports lockfile v4 (rust 1.78+)
# updated from 1.77
from rust:1.79 as builder

# install cross-compilation tools for windows gnu target
run apt-get update && apt-get install -y --no-install-recommends \
    gcc-mingw-w64-x86-64 \
    && apt-get clean && rm -rf /var/lib/apt/lists/*

# add the windows gnu target
run rustup target add x86_64-pc-windows-gnu

workdir /app

# configure cargo to use the mingw linker for the windows target
# this needs to happen *before* the build command
# create .cargo directory first if it doesn't exist from COPY
run mkdir -p .cargo
run echo '[target.x86_64-pc-windows-gnu]' >> .cargo/config.toml && \
    echo 'linker = "x86_64-w64-mingw32-gcc"' >> .cargo/config.toml && \
    echo 'ar = "x86_64-w64-mingw32-ar"' >> .cargo/config.toml

# --- Simplified Copy ---
# copy the entire build context (project directory) into the container's workdir
# note: this is less efficient for docker layer caching than copying manifests first
copy . .

# build the final linux binary
# use the copied source code directly
run cargo build --release --locked --target-dir /app/target/linux_build

# build the final windows binary
# use the copied source code directly
run cargo build --release --locked --target x86_64-pc-windows-gnu --target-dir /app/target/windows_build


# === Stage 2: Final Image (Contains both binaries) ===
# use a minimal base image - debian is used here for simplicity
from debian:stable-slim

# copy the compiled linux binary from the builder stage
copy --from=builder /app/target/linux_build/release/git-changes-rs /usr/local/bin/git-changes-rs-linux

# copy the compiled windows binary from the builder stage
copy --from=builder /app/target/windows_build/x86_64-pc-windows-gnu/release/git-changes-rs.exe /usr/local/bin/git-changes-rs.exe

