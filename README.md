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

# References
- https://revers.engineering/7-days-to-virtualization-a-series-on-hypervisor-development/
- https://github.com/memN0ps/illusion-rs
- https://rayanfam.com/topics/hypervisor-from-scratch-part-7/
