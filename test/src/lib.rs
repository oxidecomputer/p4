#![allow(clippy::too_many_arguments)]

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
mod dynamic_router;
#[cfg(test)]
mod headers;
#[cfg(test)]
mod hub;
#[cfg(test)]
mod ipv6;
#[cfg(test)]
mod mac_rewrite;
#[cfg(test)]
mod range;
#[cfg(test)]
mod table_in_egress_and_ingress;
#[cfg(test)]
mod vlan;

pub mod data;
pub mod packet;
pub mod softnpu;
