In order to flash the device, execute the following command

```bash
openocd  -c "debug_level 2" -f utils/wch-riscv.cfg -c init -c halt -c "program {target/riscv32imfc-unknown-none-elf/debug/dumper} verify reset" -c shutdown
```