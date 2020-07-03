FROM alpine:edge

RUN apk add --no-cache \
	clang gcc g++ make cmake curl \
	openjdk8-jre-base \
	rustup && \
	rustup-init --default-toolchain nightly -y -t wasm32-wasi
ENV PATH /root/.rustup/toolchains/nightly-x86_64-unknown-linux-musl/bin/:/root/.cargo/bin/:${PATH}
