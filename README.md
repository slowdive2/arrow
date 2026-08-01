arrow is a rust hypervisor... 

currently traps-and-emulates a few categories (see hypervisor/src/exit/), spoofs hypervisor cpuid checks,
supports msr r/w, vmx -> #UD exception

WIP:
ept
formal verification

# References
- https://revers.engineering/7-days-to-virtualization-a-series-on-hypervisor-development/
- https://github.com/memN0ps/illusion-rs
