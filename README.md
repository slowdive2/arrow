arrow is a rust hypervisor... 

currently traps-and-emulates a few categories (see hypervisor/src/exit/), spoofs hypervisor cpuid checks,
supports msr r/w, vmx -> #UD exception

ept is synchronized across all cores, MTRR-derived leaf types,
on-demand 2 MiB-to-4 KiB splits, `INVEPT`, and an execute monitor (1 shot)


# References
- https://revers.engineering/7-days-to-virtualization-a-series-on-hypervisor-development/
- https://github.com/memN0ps/illusion-rs
- https://rayanfam.com/topics/hypervisor-from-scratch-part-7/
