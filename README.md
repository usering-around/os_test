An attempt at an OS in order to learn about x86_64 and kernels. Currently extremely limited; only memory allocation and screen are available.

Building: Clone the repo and install rust, qemu-system-x86_64, xorriso and python. Then run: <br>
`cargo run` <br>
or 
`make qemu` <br>
to run it in an qemu. <br>
Use `make clean` + `cargo clean` to clean up build artifacts. <br>
To test, use: <br>
`cargo test`

