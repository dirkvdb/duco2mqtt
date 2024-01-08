#![warn(clippy::unwrap_used)]
pub mod bridge;
mod duconode;
pub mod modbus;
pub mod mqtt;
mod nodetypes;

extern crate num;
#[macro_use]
extern crate num_derive;

use std::{array::TryFromSliceError, fmt, net::AddrParseError, str::Utf8Error};

#[derive(Debug)]
pub enum MqttBridgeError {
    NetworkError(String),
    RuntimeError(String),
    ParseError,
}

impl fmt::Display for MqttBridgeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MqttBridgeError::NetworkError(str) => write!(f, "Network Error {}", str),
            MqttBridgeError::RuntimeError(str) => write!(f, "Runtime Error {}", str),
            MqttBridgeError::ParseError => write!(f, "Parse Error"),
        }
    }
}

impl From<std::io::Error> for MqttBridgeError {
    fn from(err: std::io::Error) -> Self {
        MqttBridgeError::NetworkError(format!("IO: {}", err))
    }
}

impl From<std::num::ParseIntError> for MqttBridgeError {
    fn from(_: std::num::ParseIntError) -> Self {
        MqttBridgeError::ParseError
    }
}

impl From<String> for MqttBridgeError {
    fn from(str: String) -> Self {
        MqttBridgeError::RuntimeError(str)
    }
}

impl From<AddrParseError> for MqttBridgeError {
    fn from(_: AddrParseError) -> Self {
        MqttBridgeError::NetworkError(String::from("Invalid address"))
    }
}

impl From<TryFromSliceError> for MqttBridgeError {
    fn from(_: TryFromSliceError) -> Self {
        MqttBridgeError::ParseError
    }
}

impl From<Utf8Error> for MqttBridgeError {
    fn from(_: Utf8Error) -> Self {
        MqttBridgeError::ParseError
    }
}

impl From<rumqttc::v5::ClientError> for MqttBridgeError {
    fn from(err: rumqttc::v5::ClientError) -> Self {
        MqttBridgeError::RuntimeError(format!("MQTT Error: {err}"))
    }
}

impl From<rumqttc::v5::ConnectionError> for MqttBridgeError {
    fn from(err: rumqttc::v5::ConnectionError) -> Self {
        MqttBridgeError::RuntimeError(format!("MQTT Connection error: {err}"))
    }
}
