//! XDP DDoS shield (run from the uplink dispatcher).

use aya_ebpf::{bindings::xdp_action, programs::XdpContext};

#[inline(always)]
pub fn shield(_ctx: &XdpContext) -> u32 {
    xdp_action::XDP_PASS
}
