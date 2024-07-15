use core::fmt;
use std::collections::HashMap;

use anyhow::{anyhow, Context};
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
pub enum NodeValue {
    String(String),
    Number(i64),
}

impl fmt::Display for NodeValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            NodeValue::String(s) => write!(f, "{}", s),
            NodeValue::Number(n) => write!(f, "{}", n),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Clone)]
pub struct NodeStatusField {
    pub Val: NodeValue,
}

impl From<&str> for NodeStatusField {
    fn from(val: &str) -> NodeStatusField {
        NodeStatusField {
            Val: NodeValue::String(String::from(val)),
        }
    }
}

impl From<i64> for NodeStatusField {
    fn from(val: i64) -> NodeStatusField {
        NodeStatusField {
            Val: NodeValue::Number(val),
        }
    }
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize, Clone)]
pub struct NodeInfo {
    pub Node: u16,
    pub General: HashMap<String, NodeStatusField>,
    pub Ventilation: HashMap<String, NodeStatusField>,
    pub Sensor: Option<HashMap<String, NodeStatusField>>,
}

#[allow(non_snake_case)]
#[derive(Debug, Serialize)]
pub struct NodeAction {
    pub Action: String,
    pub Val: String,
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

pub async fn perform_action(client: &reqwest::Client, addr: &str, node: u16, action: NodeAction) -> Result<()> {
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
                    let mut general: Option<HashMap<String, NodeStatusField>> = None;
                    let mut ventilation: Option<HashMap<String, NodeStatusField>> = None;
                    let mut sensor: Option<HashMap<String, NodeStatusField>> = None;

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

impl<'de> serde::Deserialize<'de> for NodeValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct NodeValueVisitor;

        impl<'de> serde::de::Visitor<'de> for NodeValueVisitor {
            type Value = NodeValue;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "an integer or a string value")
            }

            fn visit_str<E: serde::de::Error>(self, s: &str) -> std::result::Result<NodeValue, E> {
                Ok(NodeValue::String(s.to_string()))
            }

            fn visit_i64<E: serde::de::Error>(self, n: i64) -> std::result::Result<NodeValue, E> {
                Ok(NodeValue::Number(n))
            }

            fn visit_u64<E: serde::de::Error>(self, n: u64) -> std::result::Result<NodeValue, E> {
                Ok(NodeValue::Number(n as i64))
            }

            fn visit_i32<E: serde::de::Error>(self, n: i32) -> std::result::Result<NodeValue, E> {
                Ok(NodeValue::Number(n as i64))
            }

            fn visit_u32<E: serde::de::Error>(self, n: u32) -> std::result::Result<NodeValue, E> {
                Ok(NodeValue::Number(n as i64))
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
    fn test_parse_node_actions() {
        let json_repsonse = include_bytes!("../test/data/node_actions.json");

        let node_actions = parse_node_actions(json_repsonse).unwrap();
        assert_eq!(node_actions.len(), 5);
    }

    #[test]
    fn test_node_value_compare() {
        let n1 = NodeValue::Number(1);
        let n2 = NodeValue::Number(2);
        let n3 = NodeValue::Number(1);
        let s1 = NodeValue::String("foo".to_string());
        let s2 = NodeValue::String("foo".to_string());
        let s3 = NodeValue::String("foo1".to_string());

        assert_eq!(s1, s2);
        assert_eq!(n1, n3);

        assert!(!(s1 != s2));
        assert!(s1 != n1);
        assert!(s1 != s3);
        assert!(n1 != n2);
    }
}
