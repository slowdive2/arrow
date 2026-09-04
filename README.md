# 04 Sept. : arrow is being extended privately for future project; no longer maintained

# Arrow is a minimal hypervisor written in Rust

Currently supports VM-exit handling, MSR interception, exception injection, and a synchronized EPT implementation.

The EPT subsystem currently identity-maps & derives memory-types from MTRRs, maintaining similarity with the original guest being hyperjacked. 2mb - 4kB page splitting + execution monitoring is currently supported. 
[EPT implementation here](hypervisor/src/ept)

# WIP:
- vmx non-root (user level) hooking calls &&
- something more interesting than 1 shot exec monitoring

# Long term:
- formal verification experiments
- various stealth-based hooking techniques
- debugging capabilities

# Build

64-bit Windows with the MSVC Rust toolchain, Visual Studio Build Tools, Windows SDK, and the matching Windows Driver Kit. The WDK installation must include the kernel-mode `km` headers

```powershell
rustup component add rustfmt
cargo fmt --all -- --check
cargo check --workspace
cargo test -p hypervisor --lib
```


# References
- https://revers.engineering/7-days-to-virtualization-a-series-on-hypervisor-development/
- https://github.com/memN0ps/illusion-rs
- https://rayanfam.com/topics/hypervisor-from-scratch-part-7/
