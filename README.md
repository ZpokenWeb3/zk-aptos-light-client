# ZK Aptos light client

This is a light client for the Aptos blockchain, written in Rust, and located within the workspace defined in this project.
This project also includes a proving server that provides a REST API for generating proofs for the light client.
## Quick Start

First, make sure [rustup] is installed. The
[`rust-toolchain.toml`][rust-toolchain] file will be used by `cargo` to
automatically install the correct version.

To build all methods and execute the method within the zkVM, run the following
command:

```bash
cargo run
```

This is an empty template, and so there is no expected output (until you modify
the code).

### Executing the Project Locally in Development Mode

During development, faster iteration upon code changes can be achieved by leveraging [dev-mode], we strongly suggest activating it during your early development phase. Furthermore, you might want to get insights into the execution statistics of your project, and this can be achieved by specifying the environment variable `RUST_LOG="[executor]=info"` before running your project.

Put together, the command to run your project in development mode while getting execution statistics is:

```bash
RUST_LOG="[executor]=info" RISC0_DEV_MODE=1 cargo run
```


## Directory Structure

It is possible to organize the files for these components in various ways.
However, in this starter template we use a standard directory structure for zkVM
applications, which we think is a good starting point for your applications.

```text
project_name
├── Cargo.toml
├──   core                          <-- [Library that contains the data structures and utilities used by the light client.]
├── host
│   ├── Cargo.toml
│   └── src
│       └── main.rs                 <-- [Host code goes here]
└── guests
    ├── Cargo.toml
    ├── build.rs
    ├── inclusion
    │   ├── Cargo.toml
    │   └── src
    │       └── main.rs             <-- [Inclusion program]
    ├── epoch-change
    │   ├── Cargo.toml
    │   └── src
    │       └── main.rs             <-- [Epoch changing program]
    └── src
        └── lib.rs
```

## Acknowledgements

This project is a fork of the work done by [Argument Computer Corporation](https://argument.xyz), which implemented a variety of ZK light clients. You can find more of their implementations through [the provided link](https://github.com/argumentcomputer/zk-light-clients). The contributions of numerous organizations reflect the belief that zero-knowledge cryptography is the future, further reinforcing the importance of innovation in this field. They demonstrate that we can enhance the ZK community in blockchain using different approaches.
