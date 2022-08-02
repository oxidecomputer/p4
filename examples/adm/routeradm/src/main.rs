use std::fs::OpenOptions;
use std::io::Write;
use std::net::IpAddr;

use propolis::hw::virtio::softnpu::{ManagementMessage, ManagementFunction};
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
    }

}

fn main() {

    let cli = Cli::parse();

    match cli.command {
        Commands::AddRoute{ destination, mask, port } => {

            let mut keyset_data: Vec<u8> = match destination {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            keyset_data.push(mask);

            let parameter_data = vec![port];

            send(ManagementMessage{
                function: ManagementFunction::TableAdd,
                table: 1,
                action: 1,
                keyset_data,
                parameter_data,
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
                table: 1,
                keyset_data,
                .. Default::default()
            });
        }
        Commands::AddAddress{ address } => {
            let keyset_data: Vec<u8> = match address {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            send(ManagementMessage{
                function: ManagementFunction::TableAdd,
                table: 0,
                action: 0,
                keyset_data,
                .. Default::default()
            });
        }
        Commands::RemoveAddress{ address } => {
            let keyset_data: Vec<u8> = match address {
                IpAddr::V4(addr) => addr.octets().into(),
                IpAddr::V6(addr) => addr.octets().into(),
            };
            send(ManagementMessage{
                function: ManagementFunction::TableRemove,
                table: 0,
                keyset_data,
                .. Default::default()
            });
        }
    }

}

fn send(msg: ManagementMessage) {

    let buf = msg.to_wire();

    let mut f = OpenOptions::new()
        .write(true)
        .open("/dev/tty03")
        .unwrap();

    f.write_all(&buf).unwrap();
}
