#![warn(clippy::unwrap_used)]
use thiserror::Error;

pub mod bridge;
mod ducoapi;
mod duconodetypes;
mod duxoboxnode;
mod hassdiscovery;
pub mod mqtt;

extern crate num;
#[macro_use]
extern crate num_derive;

use std::{net::AddrParseError, str::Utf8Error};

#[derive(Debug, Error)]
pub enum Error {
    #[error("Network Error {0}")]
    Network(#[from] std::io::Error),
    #[error("Error {0}")]
    Runtime(String),
    #[error("Parse error {0}")]
    ParseError(#[from] std::num::ParseIntError),
    #[error("Utf8 error {0}")]
    Utf8Error(#[from] Utf8Error),
    #[error("Invalid address {0}")]
    InvalidAddress(#[from] AddrParseError),
    #[error("MQTT error {0}")]
    MqttClientError(#[from] rumqttc::v5::ClientError),
    #[error("MQTT error {0}")]
    MqttConnectionError(#[from] rumqttc::v5::ConnectionError),
    #[error("Serialization error {0}")]
    SerializationError(#[from] serde_json::Error),
    #[error("Request error {0}")]
    RequestError(#[from] reqwest::Error),
}

type Result<T> = std::result::Result<T, Error>;
