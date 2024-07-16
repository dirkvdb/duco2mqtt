use core::fmt;
use std::collections::HashMap;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{Error, Result};

#[derive(Debug, Deserialize)]
enum NodeType {
    #[serde(rename = "BOX")]
    Box,
    #[serde(rename = "UCCO2")]
    Co2,
}

#[allow(non_snake_case)]
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum StatusValue {
    String(String),
    Number(i64),
}

impl fmt::Display for StatusValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            StatusValue::String(s) => write!(f, "{}", s),
            StatusValue::Number(n) => write!(f, "{}", n),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Clone)]
pub struct StatusField {
    pub Val: StatusValue,
}

impl From<&str> for StatusField {
    fn from(val: &str) -> StatusField {
        StatusField {
            Val: StatusValue::String(String::from(val)),
        }
    }
}

impl From<i64> for StatusField {
    fn from(val: i64) -> StatusField {
        StatusField {
            Val: StatusValue::Number(val),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Clone)]
pub struct NodeInfo {
    pub Node: u16,
    pub General: HashMap<String, StatusField>,
    pub Ventilation: HashMap<String, StatusField>,
    pub Sensor: Option<HashMap<String, StatusField>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Clone)]
pub struct DeviceInfo {
    pub General: HashMap<String, StatusField>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize)]
pub struct NodeEnumAction {
    pub Action: String,
    pub Val: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize)]
pub struct NodeBoolAction {
    pub Action: String,
    pub Val: bool,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct NodeActionDescription {
    pub Action: String,
    pub ValType: String,
    pub Enum: Option<Vec<String>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct NodeActions {
    pub Node: u16,
    pub Actions: Vec<NodeActionDescription>,
}

pub async fn perform_action<T: serde::Serialize>(
    client: &reqwest::Client,
    addr: &str,
    node: u16,
    action: T,
) -> Result<()> {
    let url = format!("https://{}/action/nodes/{}", addr, node);
    client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(serde_json::to_string(&action)?)
        .send()
        .await
        .context("Failed to perform node action")?;
    Ok(())
}

pub async fn get_device_info(client: &reqwest::Client, addr: &str) -> Result<DeviceInfo> {
    let url = format!("https://{}/info", addr);
    let response = client.get(&url).send().await.context("Failed to obtain device info")?;
    let json_data = response.bytes().await?;
    parse_device_info(&json_data)
}

pub async fn get_nodes(client: &reqwest::Client, addr: &str) -> Result<Vec<NodeInfo>> {
    let url = format!("https://{}/info/nodes", addr);
    let response = client.get(&url).send().await.context("Failed to obtain nodes")?;
    let json_data = response.bytes().await?;
    let mut nodes = parse_node_info(&json_data)?;
    nodes.sort_by(|a, b| a.Node.cmp(&b.Node));
    Ok(nodes)
}

pub async fn get_node_actions(client: &reqwest::Client, addr: &str) -> Result<Vec<NodeActions>> {
    let url = format!("https://{}/action/nodes", addr);
    let response = client.get(&url).send().await.context("Failed to obtain node actions")?;
    let json_data = response.bytes().await?;
    let mut nodes = parse_node_actions(&json_data)?;

    nodes.sort_by(|a, b| a.Node.cmp(&b.Node));

    Ok(nodes)
}

pub fn parse_node_info(json_data: &[u8]) -> Result<Vec<NodeInfo>> {
    let mut data: HashMap<&str, serde_json::Value> = serde_json::from_slice(json_data)?;

    let mut nodes = Vec::new();
    for (&k, values) in data.iter_mut() {
        if k == "Nodes" {
            if let Some(node_values) = values.as_array() {
                for node in node_values {
                    let mut node_id: Option<u16> = None;
                    let mut general: Option<HashMap<String, StatusField>> = None;
                    let mut ventilation: Option<HashMap<String, StatusField>> = None;
                    let mut sensor: Option<HashMap<String, StatusField>> = None;

                    if node.is_object() {
                        for (k, v) in node.as_object().expect("unexpected node").iter() {
                            match k.as_str() {
                                "Node" => {
                                    node_id = Some(
                                        v.as_u64()
                                            .ok_or_else(|| Error::Runtime("Invalid node id".to_string()))?
                                            as u16,
                                    );
                                }
                                "General" => {
                                    general = Some(serde_json::from_value(v.clone())?);
                                }
                                "Ventilation" => {
                                    ventilation = Some(serde_json::from_value(v.clone())?);
                                }
                                "Sensor" => {
                                    sensor = Some(serde_json::from_value(v.clone())?);
                                }
                                _ => {}
                            };
                        }
                    }

                    nodes.push(NodeInfo {
                        Node: node_id.ok_or_else(|| Error::Runtime("Missing node id".to_string()))?,
                        General: general.ok_or_else(|| Error::Runtime("Missing general object".to_string()))?,
                        Ventilation: ventilation
                            .ok_or_else(|| Error::Runtime("Missing ventilation object".to_string()))?,
                        Sensor: sensor,
                    });
                }
            }
        }
    }

    Ok(nodes)
}

pub fn parse_device_info(json_data: &[u8]) -> Result<DeviceInfo> {
    let mut data: HashMap<&str, serde_json::Value> = serde_json::from_slice(json_data)?;

    let mut device_info = DeviceInfo {
        General: HashMap::new(),
    };

    for (&k, values) in data.iter_mut() {
        if k == "General" || k == "HeatRecovery" {
            for (group, val) in values.as_object().expect("unexpected general object") {
                for (key, value) in val.as_object().expect("unexpected general object").iter() {
                    if !value.is_array() {
                        device_info.General.insert(
                            format!("{}/{}/{}", k, group, key),
                            serde_json::from_value(value.clone())?,
                        );
                    }
                }
            }
        }
    }

    Ok(device_info)
}

pub fn parse_node_actions(json_data: &[u8]) -> Result<Vec<NodeActions>> {
    let data: HashMap<&str, serde_json::Value> = serde_json::from_slice(json_data)?;

    let mut node_actions = Vec::new();
    for (&k, value) in data.iter() {
        if k == "Nodes" {
            node_actions = serde_json::from_value(value.clone())?;
        }
    }

    Ok(node_actions)
}

impl<'de> serde::Deserialize<'de> for StatusValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NodeValueVisitor;

        impl<'de> serde::de::Visitor<'de> for NodeValueVisitor {
            type Value = StatusValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "an integer or a string value")
            }

            fn visit_str<E: serde::de::Error>(self, s: &str) -> std::result::Result<StatusValue, E> {
                Ok(StatusValue::String(s.to_string()))
            }

            fn visit_i64<E: serde::de::Error>(self, n: i64) -> std::result::Result<StatusValue, E> {
                Ok(StatusValue::Number(n))
            }

            fn visit_u64<E: serde::de::Error>(self, n: u64) -> std::result::Result<StatusValue, E> {
                Ok(StatusValue::Number(n as i64))
            }

            fn visit_i32<E: serde::de::Error>(self, n: i32) -> std::result::Result<StatusValue, E> {
                Ok(StatusValue::Number(n as i64))
            }

            fn visit_u32<E: serde::de::Error>(self, n: u32) -> std::result::Result<StatusValue, E> {
                Ok(StatusValue::Number(n as i64))
            }
        }

        deserializer.deserialize_any(NodeValueVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_node_info() {
        let json_repsonse = include_bytes!("../test/data/info_nodes.json");

        let nodes = parse_node_info(json_repsonse).unwrap();
        assert_eq!(nodes.len(), 5);
    }

    #[test]
    fn test_parse_device_info() {
        let json_repsonse = include_bytes!("../test/data/info.json");

        let device = parse_device_info(json_repsonse).unwrap();
        assert_eq!(
            device.General["General/Board/BoxName"].Val,
            StatusValue::String("ENERGY".to_string())
        );
        assert_eq!(
            device.General["General/Board/BoxSubTypeName"].Val,
            StatusValue::String("PREMIUM_400_2ZH_R".to_string())
        );
        assert_eq!(
            device.General["HeatRecovery/General/TimeFilterRemain"].Val,
            StatusValue::Number(59)
        );
    }

    #[test]
    fn test_parse_node_actions() {
        let json_repsonse = include_bytes!("../test/data/node_actions.json");

        let node_actions = parse_node_actions(json_repsonse).unwrap();
        assert_eq!(node_actions.len(), 5);
    }

    #[test]
    fn test_node_value_compare() {
        let n1 = StatusValue::Number(1);
        let n2 = StatusValue::Number(2);
        let n3 = StatusValue::Number(1);
        let s1 = StatusValue::String("foo".to_string());
        let s2 = StatusValue::String("foo".to_string());
        let s3 = StatusValue::String("foo1".to_string());

        assert_eq!(s1, s2);
        assert_eq!(n1, n3);

        assert!(!(s1 != s2));
        assert!(s1 != n1);
        assert!(s1 != s3);
        assert!(n1 != n2);
    }
}
