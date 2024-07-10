use std::collections::HashMap;

use serde::Deserialize;

use crate::{Error, Result};

#[derive(Debug, Deserialize)]
enum NodeType {
    #[serde(rename = "BOX")]
    Box,
    #[serde(rename = "UCCO2")]
    Co2,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct StringValue {
    pub Val: String,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct NumberValue {
    #[serde(rename = "Val")]
    pub Val: i64,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct General {
    pub Type: StringValue,
    pub SubType: NumberValue,
    pub NetworkType: StringValue,
    pub Parent: NumberValue,
    pub Asso: NumberValue,
    pub Name: StringValue,
    pub Identify: NumberValue,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct VentilationInfo {
    pub State: StringValue,
    pub TimeStateRemain: NumberValue,
    pub TimeStateEnd: NumberValue,
    pub Mode: StringValue,
    pub FlowLvlTgt: Option<NumberValue>,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
struct SensorInfo {
    IaqCo2: NumberValue,
}

#[allow(non_snake_case)]
#[derive(Debug, Deserialize)]
pub struct NodeInfo {
    pub Node: u16,
    pub General: General,
    pub Ventilation: VentilationInfo,
    pub Sensor: Option<SensorInfo>,
}

pub async fn get_nodes(client: &reqwest::Client, addr: &str) -> Result<Vec<NodeInfo>> {
    let url = format!("https://{}/info/nodes", addr);
    let response = client.get(&url).send().await?;
    let json_data = response.bytes().await?;
    parse_node_info(&json_data)
}

pub fn parse_node_info(json_data: &[u8]) -> Result<Vec<NodeInfo>> {
    let mut data: HashMap<&str, serde_json::Value> = serde_json::from_slice(json_data)?;

    let mut nodes = Vec::new();
    for (&k, values) in data.iter_mut() {
        if k == "Nodes" {
            if let Some(node_values) = values.as_array() {
                for node in node_values {
                    let mut node_id: Option<u16> = None;
                    let mut general: Option<General> = None;
                    let mut ventilation: Option<VentilationInfo> = None;
                    let mut sensor: Option<SensorInfo> = None;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_node_info() {
        let json_repsonse = include_bytes!("../test/data/info_nodes.json");

        let nodes = parse_node_info(json_repsonse).unwrap();
        assert_eq!(nodes.len(), 5);
    }
}
