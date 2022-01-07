// Copyright 2021 The Ronvoy Authors. All rights reserved.
// Use of this source code is governed by the Apache License,
// Version 2.0, that can be found in the LICENSE file.

use std::net::{AddrParseError, SocketAddr};

use envoy_control_plane::envoy::config::core::v3::{
    address::Address as V3InnerAddress, Address as V3Address,
};

use envoy_control_plane::envoy::config::core::v3::socket_address::PortSpecifier;
use std::error::Error as StdError;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    UnsupportedAddress(String),
    MissingValue, // protobuf missing value
    MissingPort,
    PortTooBig(u32),
    Parse(AddrParseError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnsupportedAddress(msg) => write!(f, "unsupported address type: \"{}\"", msg),
            Error::MissingValue => write!(
                f,
                "missing value (likely mistake in creating/serializing protobuf"
            ),
            Error::MissingPort => write!(
                f,
                "missing port (likely mistake in creating/serializing protobuf"
            ), //
            Error::PortTooBig(port) => {
                write!(f, "port value {} bigger than max port of 2^16", port)
            }
            Error::Parse(addr_err) => write!(f, "error parsing socket addr: {}", addr_err),
        }
    }
}

impl StdError for Error {}

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
