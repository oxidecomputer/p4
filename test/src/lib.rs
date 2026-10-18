// Copyright 2026 Oxide Computer Company

#![allow(clippy::too_many_arguments)]

#[cfg(test)]
mod action_param_offset;
#[cfg(test)]
mod arithmetic;
#[cfg(test)]
mod basic_router;
#[cfg(test)]
mod controller_multiple_instantiation;
#[cfg(test)]
mod decap;
#[cfg(test)]
mod disag_router;
#[cfg(test)]
mod dload;
#[cfg(test)]
mod double_instantiation;
#[cfg(test)]
mod dynamic_router;
#[cfg(test)]
mod dynamic_router_noaddr;
#[cfg(test)]
mod headers;
#[cfg(test)]
mod hub;
#[cfg(test)]
mod ipv6;
#[cfg(test)]
mod mac_rewrite;
#[cfg(test)]
mod mcast;
#[cfg(test)]
mod metadata_names;
#[cfg(test)]
mod range;
#[cfg(test)]
mod shift;
#[cfg(test)]
mod slice_assign;
#[cfg(test)]
mod slice_assign_empty;
#[cfg(test)]
mod slice_read;
#[cfg(test)]
mod table_in_egress_and_ingress;
#[cfg(test)]
mod vlan;
#[cfg(test)]
mod vlan_trunk;
#[cfg(test)]
mod vrf_router;

pub mod data;
pub mod packet;
pub mod softnpu;
