## Windows

Testing via Wine:

```
sudo apt install gcc-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
cargo build --target=x86_64-pc-windows-gnu  --example hello
```

Then, from an X session of some kind:

```
wineconsole cmd.exe
```

and from there you can launch the generated .exe files; they are found under `target/x86_64-pc-windows-gnu/debug`


