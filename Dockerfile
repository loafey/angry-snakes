FROM rustlang/rust:nightly as builder
WORKDIR /usr/src/angry-snakes
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY frontend/ frontend/
COPY crates/ crates/
COPY sample-client/ sample-client/
RUN cargo run -- schema
# RUN rustup target add x86_64-unknown-linux-musl
RUN cargo install --path . # --target=x86_64-unknown-linux-musl

FROM ubuntu:latest
RUN apt-get update && apt-get install -y curl && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/* /usr/local/bin
EXPOSE 8000
CMD ["angry-snakes"]
