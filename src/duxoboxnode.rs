use std::{collections::HashMap, str::FromStr};

use crate::{
    ducoapi::{self, NodeAction, NodeActionDescription, NodeActions, NodeInfo, NodeStatusField, NodeValue},
    duconodetypes::NodeType,
    mqtt::MqttData,
    Error, Result,
};

use anyhow::{anyhow, bail};

pub const UNKNOWN: &str = "UNKNOWN";

struct InfoValue {
    value: NodeValue,
    modified: bool,
}

impl InfoValue {
    fn new(value: NodeValue) -> Self {
        Self { value, modified: true }
    }

    fn set(&mut self, val: NodeValue) {
        if val != self.value {
            self.modified = true;
            self.value = val;
        }
    }

    fn modified(&self) -> bool {
        self.modified
    }

    fn get_and_reset(&mut self) -> NodeValue {
        self.modified = false;
        self.value.clone()
    }
}

pub enum DucoNodeAction {
    SetBoolean(String),
    SetEnum(String, Vec<String>),
}

pub struct DucoBoxNode {
    number: u16,
    node_type: NodeType,
    status: HashMap<String, InfoValue>,
    actions: Vec<DucoNodeAction>,
}

impl DucoBoxNode {
    pub fn create_for_node_type(node_type: NodeType, number: u16) -> DucoBoxNode {
        DucoBoxNode {
            number,
            node_type,
            status: HashMap::default(),
            actions: Vec::default(),
        }
    }

    pub fn number(&self) -> u16 {
        self.number
    }

    pub fn reset(&mut self) {
        for (_key, value) in self.status.iter_mut() {
            value.set(NodeValue::String(UNKNOWN.to_string()))
        }
    }

    pub fn topics_that_need_updating(&mut self) -> Vec<MqttData> {
        let mut topics = Vec::new();

        for (key, value) in self.status.iter_mut() {
            if value.modified() {
                let val = value.get_and_reset();
                topics.push(MqttData {
                    topic: DucoBoxNode::status_topic(self.number, key),
                    payload: val.to_string(),
                });
            }
        }

        topics
    }

    fn status_topic(node_number: u16, topic: &str) -> String {
        format!("duco_node_{}/{}", node_number, topic)
    }

    fn merge_status_values(&mut self, sub_topic: &str, values: HashMap<String, NodeStatusField>) {
        for (name, value) in values {
            let key = format!("{sub_topic}/{name}");
            if let Some(info_value) = self.status.get_mut(&key) {
                info_value.set(value.Val);
            } else {
                self.status.insert(key, InfoValue::new(value.Val));
            }
        }
    }

    pub fn update_status(&mut self, node: NodeInfo) -> Result<()> {
        assert_eq!(self.number, node.Node, "Node number mismatch");

        self.merge_status_values("general", node.General);
        self.merge_status_values("ventilation", node.Ventilation);
        if let Some(sensor) = node.Sensor {
            self.merge_status_values("sensor", sensor);
        }

        Ok(())
    }

    pub fn set_actions(&mut self, actions: NodeActions) -> Result<()> {
        self.actions = actions
            .Actions
            .into_iter()
            .map(DucoNodeAction::try_from)
            .collect::<Result<Vec<_>>>()?;

        Ok(())
    }

    fn verify_action_is_valid(&self, action: &NodeAction) -> Result<()> {
        for node_action in &self.actions {
            match node_action {
                DucoNodeAction::SetBoolean(ref action_name) => {
                    if *action_name == action.Action {
                        return Ok(());
                    }
                }
                DucoNodeAction::SetEnum(ref action_name, ref values) => {
                    if action_name == &action.Action {
                        if !values.contains(&action.Val) {
                            bail!("Invalid value for action '{}': '{}'", action.Action, action.Val);
                        }

                        return Ok(());
                    }
                }
            }
        }

        Err(anyhow!("Invalid action for node {}: '{}'", self.number, action.Action))
    }

    pub async fn process_command(&self, action: NodeAction, client: &reqwest::Client, addr: &str) -> Result<()> {
        self.verify_action_is_valid(&action)?;
        ducoapi::perform_action(client, addr, self.number(), action).await
    }
}

impl TryFrom<ducoapi::NodeInfo> for DucoBoxNode {
    type Error = anyhow::Error;

    fn try_from(node_info: ducoapi::NodeInfo) -> Result<Self> {
        let NodeValue::String(ref type_str) = node_info
            .General
            .get("Type")
            .ok_or_else(|| Error::Runtime("Type missing".to_string()))?
            .Val
        else {
            bail!("Node type is not a string value");
        };

        let node_type =
            NodeType::from_str(type_str).map_err(|_| Error::Runtime(format!("Unknown node type: {}", type_str)))?;

        let mut node = DucoBoxNode::create_for_node_type(node_type, node_info.Node);
        node.update_status(node_info)?;
        Ok(node)
    }
}

impl TryFrom<NodeActionDescription> for DucoNodeAction {
    type Error = anyhow::Error;

    fn try_from(action: NodeActionDescription) -> Result<Self> {
        match action.ValType.as_str() {
            "Enum" => Ok(DucoNodeAction::SetEnum(
                action.Action,
                action
                    .Enum
                    .ok_or_else(|| Error::Runtime("Enum values missing for action".to_string()))?,
            )),
            "Boolean" => Ok(DucoNodeAction::SetBoolean(action.Action)),
            _ => Err(anyhow!("Unsupported action type '{}'", action.ValType)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_info_value() {
        let mut val = InfoValue::new(NodeValue::String("foo".to_string()));
        assert!(val.modified());
        assert_eq!(val.get_and_reset(), NodeValue::String("foo".to_string()));
        assert!(!val.modified());
    }

    #[test]
    fn test_duco_node_action() {
        let action = NodeActionDescription {
            Action: "foo".to_string(),
            ValType: "Enum".to_string(),
            Enum: Some(vec!["bar".to_string()]),
        };

        let duco_action = DucoNodeAction::try_from(action).unwrap();
        if let DucoNodeAction::SetEnum(action, values) = duco_action {
            assert_eq!(action, "foo");
            assert_eq!(values, vec!["bar".to_string()]);
        } else {
            panic!("Unexpected action type");
        }
    }

    #[test]
    fn test_ducobox_node() {
        let node_info = NodeInfo {
            Node: 1,
            General: HashMap::from([
                ("Type".to_string(), NodeStatusField::from("BOX")),
                ("SubType".to_string(), NodeStatusField::from(1)),
            ]),
            Ventilation: HashMap::new(),
            Sensor: None,
        };

        let mut node = DucoBoxNode::try_from(node_info).unwrap();
        assert_eq!(node.number(), 1);

        let mut topics = node.topics_that_need_updating();
        topics.sort();
        assert_eq!(
            topics,
            vec![
                MqttData::new("node_1/general/SubType", "1"),
                MqttData::new("node_1/general/Type", "BOX")
            ]
        );

        let node_info_update = NodeInfo {
            Node: 1,
            General: HashMap::from([
                ("SubType".to_string(), NodeStatusField::from(2)),
                ("Type".to_string(), NodeStatusField::from("BOX")),
            ]),
            Ventilation: HashMap::new(),
            Sensor: None,
        };

        node.update_status(node_info_update.clone()).unwrap();
        assert_eq!(
            node.topics_that_need_updating(),
            vec![MqttData::new("node_1/general/SubType", "2"),]
        );

        node.update_status(node_info_update.clone()).unwrap();
        assert!(node.topics_that_need_updating().is_empty(),);
    }
}
