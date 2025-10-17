import sys
import os
if __name__ == "__main__":
    make_target = sys.argv[1]
    # cargo run passes the bin path after the make flag
    bin_path = sys.argv[2]
    command = f"make {make_target} BIN_PATH={bin_path}"
    print(command)
    # disable display when we're doing cargo test'
    if not bin_path.endswith("os_test"):
        qemu_args = "-display none -device isa-debug-exit,iobase=0xf4,iosize=0x04"
        command = f"{command} QEMU_ARGS=\"{qemu_args}\""
    os.system(command)
