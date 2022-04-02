## Building on Windows

Pre-requisites:

* You'll need to install [Visual Studio](https://visualstudio.microsoft.com/), the community edition works fine. Install the `Desktop development with C++` workload.
* Install `rustup` to get the `rust` compiler installed on your system.
  [Install rustup](https://www.rust-lang.org/en-US/install.html). You don't need to install 
  the `Visual Studio C++ Build tools` if you already installed Visual Studio.
* Rust version 1.56 or later is required
* Install [Git for windows](https://git-scm.com/download/win).
* Install and configure [vcpkg](https://github.com/microsoft/vcpkg#quick-start-windows),
  including running the `integrate install` command.

After all pre-requisites are installed and configured, you'll need to install `OpenSSL` from
`vcpkg`. Open a new cmd/powershell and navigate to the directory you installed `vcpkg`,
which contains the `vcpkg.exe` executable, and execute the command:

```
vcpkg.exe install openssl:x64-windows-static
```

Now, in order to build with cargo, first define an environment variable to configure OpenSSL.

If you're using CMD:

```cmd
set OPENSSL_NO_VENDOR=1
```

Or powershell:

```powershell
$Env:OPENSSL_NO_VENDOR=1
```

Finally use the commands to build:

* Build in release mode: `cargo build --release`
* Build and run: `cargo run --release --bin wezterm-gui`
* Build and run showing the log output:

    ```
    cargo run --release --bin wezterm-gui -- --attach-parent-console
    ```


### Developing with VS Code

- Install the [rust-analyzer extension](https://marketplace.visualstudio.com/items?itemName=matklad.rust-analyzer).
  If you have the `Rust` extension installed, uninstall it first.
- Install nightly `rustfmt` with command:

    ```
    rustup component add rustfmt --toolchain nightly
    ```

Use the following Workspace JSON settings for the best results:

```json
{
    "[rust]": {
        "editor.formatOnSave": true
    },
    "rust-analyzer.assist.importGranularity": "module",
    "rust-analyzer.assist.importGroup": false,
    "rust-analyzer.assist.importPrefix": "crate",
    "rust-analyzer.cargo.runBuildScripts": true,
    "rust-analyzer.completion.autoimport.enable": true,
    "rust-analyzer.rustfmt.extraArgs": ["+nightly"],
    "rust-analyzer.server.extraEnv": {
        "OPENSSL_NO_VENDOR": 1
    }
}
```