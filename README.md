arrow is a rust hypervisor... 

currently traps-and-emulates a few categories (see hypervisor/src/exit/), spoofs hypervisor cpuid checks,
supports msr r/w, vmx -> #UD exception

EPT now provides a per-vCPU 512 GiB identity map, MTRR-derived leaf types,
on-demand 2 MiB-to-4 KiB splits, `INVEPT`, and a one-shot execute monitor.
See [the EPT walkthrough](hypervisor/src/ept/README.md).

WIP:
fixed-range MTRRs and shared/cross-core EPT monitoring
formal verification

# References
- https://revers.engineering/7-days-to-virtualization-a-series-on-hypervisor-development/
- https://github.com/memN0ps/illusion-rs
- https://rayanfam.com/topics/hypervisor-from-scratch-part-7/
