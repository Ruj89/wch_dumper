## Build

```bash
cargo build
```

In order to flash the device, execute the following command

```bash
openocd  -c "debug_level 2" -f utils/wch-riscv.cfg -c init -c halt -c "program {target/riscv32imfc-unknown-none-elf/debug/dumper} verify reset" -c shutdown
```

## Debug

In order to debug, launch the openocd and halt

```bash
openocd  -c "debug_level 2" -f utils/wch-riscv.cfg -c init -c halt
```

Run the "Debug on target" VSCode debug configuration or a gdb linking to the target using command line:

```bash
riscv32-unknown-elf-gdb target/riscv32imfc-unknown-none-elf/debug/dumper \
    -ex "file target/riscv32imfc-unknown-none-elf/debug/dumper" \
    -ex "target extended-remote :3333"
```