FROM rust:bullseye AS base
RUN apt-get update 
RUN apt -y install libudev-dev libusb-1.0-0-dev
RUN rustup toolchain install nightly
RUN rustup default nightly
RUN rustup component add rust-src
RUN rustup update
RUN rustup target add riscv32imafc-unknown-none-elf

FROM base AS dumper_base
RUN mkdir -p /opt/ch32-data/build/
RUN git clone https://github.com/ch32-rs/ch32-hal /opt/ch32-data/build/ch32-hal

FROM dumper_base AS develop
RUN apt install sudo
RUN addgroup developer && \
    adduser --ingroup developer --shell /bin/bash -disabled-login developer && \
    mkdir -p /home/developer && chown -R developer:developer /home/developer
RUN echo "developer ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/developer && \
    chmod 0440 /etc/sudoers.d/developer
USER developer
WORKDIR /home/developer
