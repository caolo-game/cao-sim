FROM rust:latest AS deps

RUN apt-get update
RUN apt-get install lld clang capnproto -y --fix-missing

WORKDIR /caolo

COPY ./.cargo/ ./.cargo/
RUN cargo --version

# ============= cache dependencies ============================================================
WORKDIR /caolo
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
COPY ./cao-storage-derive/ ./cao-storage-derive/
COPY ./worker/Cargo.toml ./worker/Cargo.toml
COPY ./simulation/Cargo.toml ./simulation/Cargo.toml

RUN mkdir worker/src/
RUN echo "fn main() {}" > ./worker/src/dummy.rs
RUN mkdir simulation/src/
RUN echo "fn main() {}" > ./simulation/src/dummy.rs

# Delete the build script
RUN sed -i '/build\s*=\s*\"build\.rs\"/d' simulation/Cargo.toml
# Uncomment the [[bin]] section
RUN sed -i 's/src\/main.rs/src\/dummy.rs/' worker/Cargo.toml
RUN sed -i 's/# \[\[bin]]/[[bin]]/' simulation/Cargo.toml
RUN sed -i 's/# name =/name =/' simulation/Cargo.toml
RUN sed -i 's/# path =/path =/' simulation/Cargo.toml
RUN sed -i 's/# required =/required =/' simulation/Cargo.toml
# Delete the bench section
RUN sed -i '/\[\[bench/,+2d' simulation/Cargo.toml

RUN cargo build --release --all-features


# ==============================================================================================

FROM rust:latest AS build
COPY ./.cargo/ ./.cargo/
RUN cargo --version

RUN apt-get update
RUN apt-get install lld clang capnproto postgresql postgresql-contrib sudo -y --fix-missing

WORKDIR /caolo

RUN cargo install diesel_cli --no-default-features --features=postgres --root .

ENV DATABASE_URL=postgres://postgres:postgres@localhost:5432/caolo


# copy the cache
COPY --from=deps $CARGO_HOME $CARGO_HOME
COPY --from=deps /caolo/target ./target
COPY --from=deps /caolo/Cargo.lock ./Cargo.lock

COPY ./migrations ./migrations
COPY ./bash-scripts/build.sh ./build.sh
COPY ./Cargo.toml ./Cargo.toml
COPY ./simulation/ ./simulation/
COPY ./cao-storage-derive/ ./cao-storage-derive/
COPY ./worker/ ./worker/

RUN bash ./build.sh

# ========== Copy the built binary to a scratch container, to minimize the image size ==========

FROM ubuntu:20.04
WORKDIR /caolo

RUN apt-get update -y
RUN apt-get install libssl-dev libcurl4-openssl-dev -y
# RUN apt-get install valgrind -y
# RUN apt-get install heaptrack -y

COPY ./bash-scripts/ ./
COPY ./migrations ./migrations
COPY ./diesel.toml ./diesel.toml
COPY --from=build /caolo/target/release/caolo-worker ./caolo-worker
COPY --from=build /caolo/bin/diesel ./diesel
COPY ./worker/run-profile.sh ./run-profile.sh

ENTRYPOINT [ "bash", "./run.sh" ]
