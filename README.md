arrow is a rust hypervisor... 

currently traps-and-emulates a few categories (see hypervisor/src/exit/), spoofs hypervisor cpuid checks,
supports msr r/w, vmx -> #UD exception

<<<<<<< HEAD
ept now provides a per-vCPU 512 GiB identity map, MTRR-derived leaf types,
on-demand 2 MiB-to-4 KiB splits, `INVEPT`, and an execute monitor
=======
EPT now provides one shared 512 GiB identity map, MTRR-derived leaf types,
locked 2 MiB-to-4 KiB splits, cross-cpu `INVEPT`, and a one-shot execute monitor.
>>>>>>> 5b6cc02 (sync ept)

WIP:
fixed-range MTRRs and clean unload/teardown
formal verification

# References
- https://revers.engineering/7-days-to-virtualization-a-series-on-hypervisor-development/
- https://github.com/memN0ps/illusion-rs
- https://rayanfam.com/topics/hypervisor-from-scratch-part-7/
