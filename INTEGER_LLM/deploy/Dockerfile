# Multi-Stage Build fuer Rust + Python

# --- Stage 1: Rust Builder ---
FROM rust:1.79-slim AS rust-builder
WORKDIR /build
COPY kernels/ ./kernels/
COPY runtime/ ./runtime/
COPY pipeline/ ./pipeline/
RUN cd kernels && cargo build --release
RUN cd runtime && cargo build --release
RUN cd pipeline && cargo build --release

# --- Stage 2: Python Calibrate ---
FROM python:3.11-slim AS calibrate
WORKDIR /app
COPY calibrate/requirements.txt .
RUN pip install --no-cache-dir -r requirements.txt
COPY calibrate/ ./calibrate/
COPY theta_v/ ./theta_v/

# --- Stage 3: Runtime ---
FROM debian:bookworm-slim AS runtime
WORKDIR /app
COPY --from=rust-builder /build/target/release/ ./bin/
COPY theta_v/ ./theta_v/
CMD ["./bin/integer-llm-runtime"]
