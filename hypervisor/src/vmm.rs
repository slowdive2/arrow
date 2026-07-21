pub struct Vcpu {
    vmcs : VmcsRegion,
    vmcs_physical : u64,

    vmxon : VmcsRegion,
    vmxon_physical : u64,
}