use core::fmt;
use std::collections::HashMap;

use anyhow::{Context, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::{
    Result,
    ducoboxnode::{GENERAL, HEAT_RECOVERY, SENSOR, VENTILATION},
};

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

#[derive(Debug, Deserialize, Clone)]
pub struct StatusField {
    #[serde(rename = "Val")]
    pub val: StatusValue,
}

impl From<&str> for StatusField {
    fn from(val: &str) -> StatusField {
        StatusField {
            val: StatusValue::String(val.to_string()),
        }
    }
}

impl From<i64> for StatusField {
    fn from(val: i64) -> StatusField {
        StatusField {
            val: StatusValue::Number(val),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct NodeInfo {
    #[serde(rename = "Node")]
    pub node: u16,
    #[serde(rename = "General")]
    pub general: HashMap<String, StatusField>,
    #[serde(rename = "Ventilation")]
    pub ventilation: HashMap<String, StatusField>,
    #[serde(rename = "Sensor")]
    pub sensor: Option<HashMap<String, StatusField>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DeviceInfo {
    #[serde(rename = "General")]
    pub general: HashMap<String, StatusField>,
}

#[derive(Debug, Serialize)]
pub struct NodeEnumAction {
    #[serde(rename = "Action")]
    pub action: String,
    #[serde(rename = "Val")]
    pub val: String,
}

#[derive(Debug, Serialize)]
pub struct NodeBoolAction {
    #[serde(rename = "Action")]
    pub action: String,
    #[serde(rename = "Val")]
    pub val: bool,
}

#[derive(Debug, Deserialize)]
pub struct NodeActionDescription {
    #[serde(rename = "Action")]
    pub action: String,
    #[serde(rename = "ValType")]
    pub val_type: String,
    #[serde(rename = "Enum")]
    pub values: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct NodeActions {
    #[serde(rename = "Node")]
    pub node: u16,
    #[serde(rename = "Actions")]
    pub actions: Vec<NodeActionDescription>,
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
    nodes.sort_by(|a, b| a.node.cmp(&b.node));
    Ok(nodes)
}

pub async fn get_node_actions(client: &reqwest::Client, addr: &str) -> Result<Vec<NodeActions>> {
    let url = format!("https://{}/action/nodes", addr);
    let response = client.get(&url).send().await.context("Failed to obtain node actions")?;
    let json_data = response.bytes().await?;
    let mut nodes = parse_node_actions(&json_data)?;
    nodes.sort_by(|a, b| a.node.cmp(&b.node));
    Ok(nodes)
}

fn parse_node_id(json_val: &serde_json::Value) -> Result<u16> {
    json_val
        .as_u64()
        .map(|v| v as u16)
        .ok_or_else(|| anyhow!("Invalid node id: {}", json_val))
}

fn parse_status_map(
    group: &str,
    json_val: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<HashMap<String, StatusField>> {
    Ok(serde_json::from_value(
        json_val
            .remove(group)
            .ok_or_else(|| anyhow!("Missing {} object", group))?,
    )?)
}

fn parse_optional_status_map(
    group: &str,
    json_val: &mut serde_json::Map<String, serde_json::Value>,
) -> Result<Option<HashMap<String, StatusField>>> {
    if json_val.contains_key(group) {
        Ok(Some(parse_status_map(group, json_val)?))
    } else {
        Ok(None)
    }
}

pub fn parse_node_info(json_data: &[u8]) -> Result<Vec<NodeInfo>> {
    let mut data: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(json_data)?;
    let mut json_nodes = data.remove("Nodes").ok_or_else(|| anyhow!("Missing nodes list"))?;
    let Some(node_values) = json_nodes.as_array_mut() else {
        bail!("Expected nodes to be an array: {:?}", json_nodes);
    };

    node_values
        .iter_mut()
        .map(|node| -> Result<NodeInfo> {
            let node = node.as_object_mut().ok_or_else(|| anyhow!("Invalid node object"))?;
            Ok(NodeInfo {
                node: parse_node_id(node.get("Node").ok_or_else(|| anyhow!("Missing node number"))?)?,
                general: parse_status_map(GENERAL, node)?,
                ventilation: parse_status_map(VENTILATION, node)?,
                sensor: parse_optional_status_map(SENSOR, node)?,
            })
        })
        .collect()
}

pub fn parse_device_info(json_data: &[u8]) -> Result<DeviceInfo> {
    let mut data: HashMap<&str, serde_json::Value> = serde_json::from_slice(json_data)?;

    let mut device_info = DeviceInfo {
        general: HashMap::new(),
    };

    for (&k, values) in data.iter_mut() {
        if k == GENERAL || k == HEAT_RECOVERY {
            for (group, val) in values.as_object().ok_or_else(|| anyhow!("Invalid general object"))? {
                for (key, value) in val.as_object().ok_or_else(|| anyhow!("Invalid general object"))?.iter() {
                    if value.is_array() {
                        continue;
                    }

                    device_info.general.insert(
                        format!("{}/{}/{}", k, group, key),
                        serde_json::from_value(value.clone())?,
                    );
                }
            }
        }
    }

    Ok(device_info)
}

pub fn parse_node_actions(json_data: &[u8]) -> Result<Vec<NodeActions>> {
    let mut data: HashMap<&str, serde_json::Value> = serde_json::from_slice(json_data)?;
    let json_nodes = data.remove("Nodes").ok_or_else(|| anyhow!("Missing nodes list"))?;
    Ok(serde_json::from_value(json_nodes)?)
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
            device.general["General/Board/BoxName"].val,
            StatusValue::String("ENERGY".to_string())
        );
        assert_eq!(
            device.general["General/Board/BoxSubTypeName"].val,
            StatusValue::String("PREMIUM_400_2ZH_R".to_string())
        );
        assert_eq!(
            device.general["HeatRecovery/General/TimeFilterRemain"].val,
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
