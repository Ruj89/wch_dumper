FROM rust:bookworm AS base
ENV LANG=C.UTF-8
ENV LC_ALL=C.UTF-8
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

FROM base AS riscv32_toolchain
RUN apt install -y autoconf automake autotools-dev curl python3 \
		python3-pip python3-tomli libmpc-dev libmpfr-dev libgmp-dev \
		gawk build-essential bison flex texinfo gperf libtool \
		patchutils bc zlib1g-dev libexpat-dev ninja-build git cmake \
		libglib2.0-dev libslirp-dev
RUN cd /opt && git clone https://github.com/riscv/riscv-gnu-toolchain
RUN cd /opt/riscv-gnu-toolchain && ./configure --prefix=/opt/riscv \
            --target=riscv32-unknown-elf \
            --with-arch=rv32imafc \
            --with-abi=ilp32f
RUN cd /opt/riscv-gnu-toolchain && make -j$(nproc)

FROM dumper_base AS develop
COPY --from=openocd_builder /src/openocd/src/openocd /usr/bin/openocd
COPY --from=riscv32_toolchain /opt/riscv /opt/riscv
ENV PATH=$PATH:/opt/riscv/bin

RUN apt install -y sudo libpython3.11
RUN addgroup developer && \
    adduser --ingroup developer --shell /bin/bash -disabled-login developer && \
    mkdir -p /home/developer && chown -R developer:developer /home/developer
RUN echo "developer ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/developer && \
    chmod 0440 /etc/sudoers.d/developer
RUN adduser developer plugdev
USER developer
WORKDIR /home/developer
