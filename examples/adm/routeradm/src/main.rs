use std::fs::OpenOptions;
use std::io::Write;
use std::net::{IpAddr, Ipv6Addr};
use std::io::Read;

use propolis::hw::virtio::softnpu::{
    ManagementMessage,
    ManagementFunction,
    ManagementMessageInfo,
    TableModifier,
};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[clap(version, about)]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {

    /// Add a route to the routing table.
    AddRoute {
        /// Destination address for the route.
        destination: IpAddr,

        /// Subnet mask for the destination.
        mask: u8,

        /// Outbound port for the route. 
        port: u8,

        /// Next Hop
        nexthop: IpAddr,
    },

    /// Remove a route from the routing table.
    RemoveRoute {
        /// Destination address for the route.
        destination: IpAddr,

        /// Subnet mask for the destination.
        mask: u8,
    },

    /// Add an address to the router.
    AddAddress {
        /// Address to add.
        address: IpAddr,
    },

    /// Remove an address from the router.
    RemoveAddress {
        /// Address to add.
        address: IpAddr,
    },

    /// Show port count
    PortCount,

    /// Add a static NDP entry
    AddNdpEntry {
        l3: Ipv6Addr,
        l2: MacAddr,
    },

    /// Remove a static NDP entry
    RemoveNdpEntry {
        l3: Ipv6Addr,
    }

}

#[derive(Debug)]
struct MacAddr(pub [u8;6]);

impl std::str::FromStr for MacAddr {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(":").collect();
        if parts.len() != 6 {
            return Err("Expected mac in the form aa:bb:cc:dd:ee:ff".into());
        }
        let mut result = MacAddr([0u8;6]);
        for (i, p) in parts.iter().enumerate() {
            result.0[i] = match u8::from_str_radix(p, 16) {
                Ok(n) => n, 
                Err(_) => {
                    return Err(
                        "Expected mac in the form aa:bb:cc:dd:ee:ff".into());
                }
            }
        }
        Ok(result)

    }
}

fn main() {

    let cli = Cli::parse();

    match cli.command {
        Commands::AddRoute{ destination, mask, port, nexthop } => {

            let mut keyset_data: Vec<u8> = match destination {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            keyset_data.push(mask);

            let mut parameter_data = vec![port];
            let nexthop_data: Vec<u8> = match nexthop {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            parameter_data.extend_from_slice(&nexthop_data);

            send(ManagementMessage{
                function: ManagementFunction::TableAdd,
                info: ManagementMessageInfo::TableModifier(TableModifier{
                    table: 1,
                    action: 1,
                    keyset_data,
                    parameter_data,
                })
            });

        }
        Commands::RemoveRoute{ destination, mask } => {
            let mut keyset_data: Vec<u8> = match destination {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            keyset_data.push(mask);

            send(ManagementMessage{
                function: ManagementFunction::TableRemove,
                info: ManagementMessageInfo::TableModifier(TableModifier{
                    table: 1,
                    keyset_data,
                    .. Default::default()
                })
            });
        }
        Commands::AddAddress{ address } => {
            let keyset_data: Vec<u8> = match address {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            send(ManagementMessage{
                function: ManagementFunction::TableAdd,
                info: ManagementMessageInfo::TableModifier(TableModifier{
                    table: 0,
                    action: 0,
                    keyset_data,
                    .. Default::default()
                })
            });
        }
        Commands::RemoveAddress{ address } => {
            let keyset_data: Vec<u8> = match address {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            send(ManagementMessage{
                function: ManagementFunction::TableRemove,
                info: ManagementMessageInfo::TableModifier(TableModifier{
                    table: 0,
                    keyset_data,
                    .. Default::default()
                })
            });
        }
        Commands::PortCount => {

            let mut f = OpenOptions::new()
                .read(true)
                .write(true)
                .open("/dev/tty03")
                .unwrap();

            let msg = ManagementMessage{
                function: ManagementFunction::PortCount,
                .. Default::default()
            };

            let mut buf = msg.to_wire();
            buf.push('\n' as u8);

            f.write_all(&buf).unwrap();
            f.sync_all().unwrap();

            let mut buf = [0u8; 1024];
            let n = f.read(&mut buf).unwrap();
            let radix: u16 = std::str::from_utf8(&buf[..n-1])
                .unwrap()
                .parse()
                .unwrap();
            println!("{:?}", radix);

        }
        Commands::AddNdpEntry{ l3, l2 } => {
            let keyset_data: Vec<u8> = l3.octets().into();
            let parameter_data: Vec<u8> = l2.0.into();
            send(ManagementMessage{
                function: ManagementFunction::TableAdd,
                info: ManagementMessageInfo::TableModifier(TableModifier{
                    table: 2,
                    action: 0,
                    keyset_data,
                    parameter_data,
                })
            });
        }
        Commands::RemoveNdpEntry{ l3 } => {
            let keyset_data: Vec<u8> = l3.octets().into();
            send(ManagementMessage{
                function: ManagementFunction::TableRemove,
                info: ManagementMessageInfo::TableModifier(TableModifier{
                    table: 2,
                    keyset_data,
                    .. Default::default()
                })
            });
        }

    }

}

fn send(msg: ManagementMessage) {

    let mut buf = msg.to_wire();
    buf.push('\n' as u8);

    let mut f = OpenOptions::new()
        .write(true)
        .open("/dev/tty03")
        .unwrap();

    f.write_all(&buf).unwrap();
    f.sync_all().unwrap();
}
