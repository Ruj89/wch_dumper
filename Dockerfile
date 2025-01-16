FROM rust:bullseye AS base
ENV LANG C.UTF-8
ENV LC_ALL C.UTF-8
RUN apt-get update 

FROM base AS openocd_builder
RUN apt -y install libusb-1.0-0-dev
RUN mkdir /src
RUN mkdir /patches

RUN git clone https://github.com/cjacker/wch-openocd /src/openocd
WORKDIR /src/openocd
COPY utils/0001-Fixed-compilation.patch 0001-Fixed-compilation.patch
RUN git apply 0001-Fixed-compilation.patch
RUN ./bootstrap
RUN ./configure --enable-wlinke --disable-ch347 --disable-linuxgpiod --disable-werror
RUN make

FROM base AS dumper_base
# TODO useful?? 
RUN apt -y install libudev-dev libusb-1.0-0
RUN rustup toolchain install nightly
RUN rustup default nightly
RUN rustup component add rust-src
RUN rustup update
RUN rustup target add riscv32imafc-unknown-none-elf
RUN mkdir -p /opt/ch32-data/build/
RUN git clone https://github.com/ch32-rs/ch32-hal /opt/ch32-data/build/ch32-hal

FROM dumper_base AS develop
COPY --from=openocd_builder /src/openocd/src/openocd /usr/bin/openocd
RUN apt install sudo
RUN addgroup developer && \
    adduser --ingroup developer --shell /bin/bash -disabled-login developer && \
    mkdir -p /home/developer && chown -R developer:developer /home/developer
RUN echo "developer ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/developer && \
    chmod 0440 /etc/sudoers.d/developer
RUN adduser developer plugdev
USER developer
WORKDIR /home/developer
