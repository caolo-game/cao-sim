FROM rust:latest AS build

RUN apt-get update
RUN apt-get install lld clang capnproto -y --fix-missing

WORKDIR /caolo

COPY ./.cargo/ ./.cargo/
RUN cargo --version

# ============= cache dependencies =============
WORKDIR /caolo/worker
COPY ./Cargo.lock ./Cargo.lock
COPY ./worker/Cargo.toml ./Cargo.toml
RUN mkdir src/
RUN echo "fn main() {}" > ./src/dummy.rs
RUN sed -i 's/src\/main.rs/src\/dummy.rs/' Cargo.toml
# remove 'caolo' dependencies because they change often
RUN sed -i '/caolo-sim/d' Cargo.toml
RUN sed -i '/cao-lang/d' Cargo.toml
RUN cargo build --release --all-features

WORKDIR /caolo

COPY ./simulation/ ./simulation/
COPY ./cao-storage-derive/ ./cao-storage-derive/
COPY ./worker/ ./worker/

WORKDIR /caolo/worker
RUN cargo install --path . --root . --no-default-features --features=jemallocator

# ---------- Copy the built binary to a scratch container, to minimize the image size ----------

FROM ubuntu:20.04
WORKDIR /caolo

RUN apt-get update -y
RUN apt-get install libssl-dev libcurl4-openssl-dev -y
# RUN apt-get install valgrind -y
# RUN apt-get install heaptrack -y


COPY --from=build /caolo/worker/bin/caolo-worker ./caolo-worker
# COPY ./worker/run-debug.sh ./run-debug.sh

ENTRYPOINT [ "./caolo-worker" ]
