// Copyright 2022 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::net::{AddrParseError, SocketAddr};

use envoy_control_plane::envoy::config::core::v3::{
    address::Address as V3InnerAddress, socket_address::PortSpecifier, Address as V3Address,
};

#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum Error {
    #[error("unsupported address {0}")]
    UnsupportedAddress(String),
    #[error("missing value (possibly bad protobuf/serialization)")]
    MissingValue, // protobuf missing value
    #[error("missing port (possibly bad protobuf/serialization)")]
    MissingPort,
    #[error("port {0} too big (max 2^16)")]
    PortTooBig(u32),
    #[error("parse error: {0}")]
    Parse(AddrParseError),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Address {
    Socket(SocketAddr), // TODO: Unix, pipe, etc
}

impl TryFrom<V3Address> for Address {
    type Error = Error;

    fn try_from(value: V3Address) -> Result<Self, Self::Error> {
        match value {
            V3Address { address: None } => Err(Error::MissingValue),
            V3Address {
                address: Some(inner_addr),
            } => match inner_addr {
                V3InnerAddress::SocketAddress(sa) => {
                    if let Some(PortSpecifier::PortValue(port)) = sa.port_specifier {
                        let addr = SocketAddr::new(
                            sa.address.parse().map_err(Error::Parse)?,
                            port.try_into().map_err(|_| Error::PortTooBig(port))?,
                        );
                        Ok(Address::Socket(addr))
                    } else {
                        Err(Error::MissingPort)
                    }
                }
                V3InnerAddress::Pipe(_) => Err(Error::UnsupportedAddress("pipe".to_owned())),
                V3InnerAddress::EnvoyInternalAddress(_) => Err(Error::UnsupportedAddress(
                    "envoy_internal_address".to_owned(),
                )),
            },
        }
    }
}

#[test]
fn test_try_from() {
    use envoy_control_plane::envoy::config::core::v3::{
        socket_address, EnvoyInternalAddress, Pipe, SocketAddress,
    };

    let cases: &[(V3Address, Result<Address, Error>)] = &[
        (V3Address::default(), Err(Error::MissingValue)),
        (
            V3Address {
                address: Some(V3InnerAddress::Pipe(Pipe::default())),
            },
            Err(Error::UnsupportedAddress("pipe".to_owned())),
        ),
        (
            V3Address {
                address: Some(V3InnerAddress::EnvoyInternalAddress(
                    EnvoyInternalAddress::default(),
                )),
            },
            Err(Error::UnsupportedAddress(
                "envoy_internal_address".to_owned(),
            )),
        ),
        (
            V3Address {
                address: Some(V3InnerAddress::SocketAddress(SocketAddress {
                    protocol: socket_address::Protocol::Tcp as i32,
                    address: "127.0.0.1".to_string(),
                    port_specifier: None,
                    ..Default::default()
                })),
            },
            Err(Error::MissingPort),
        ),
        (
            V3Address {
                address: Some(V3InnerAddress::SocketAddress(SocketAddress {
                    protocol: socket_address::Protocol::Tcp as i32,
                    address: "10.0.0.1".to_string(),
                    port_specifier: Some(PortSpecifier::PortValue(9900)),
                    ..Default::default()
                })),
            },
            Ok(Address::Socket("10.0.0.1:9900".parse().unwrap())),
        ),
    ];

    for (input, expected) in cases.into_iter() {
        let actual = Address::try_from(input.clone());
        assert_eq!(expected, &actual);
    }
}
