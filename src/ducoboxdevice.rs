use std::collections::HashMap;

use crate::{
    ducoapi::{self, DeviceInfo, StatusField, StatusValue},
    infovalue::{InfoValue, UNKNOWN},
    mqtt::MqttData,
    Result,
};

pub struct DucoBoxDevice {
    status: HashMap<String, InfoValue>,
}

impl DucoBoxDevice {
    pub fn new() -> Self {
        Self {
            status: HashMap::default(),
        }
    }

    pub fn reset(&mut self) {
        for (_key, value) in self.status.iter_mut() {
            value.set(StatusValue::String(UNKNOWN.to_string()))
        }
    }

    pub fn topics_that_need_updating(&mut self) -> Vec<MqttData> {
        self.status
            .iter_mut()
            .filter(|(_key, value)| value.is_modified())
            .map(|(key, value)| MqttData {
                topic: key.clone(),
                payload: value.get_and_reset().to_string(),
            })
            .collect()
    }

    fn merge_status_values(&mut self, values: HashMap<String, StatusField>) {
        for (name, value) in values {
            match self.status.get_mut(&name) {
                Some(info_value) => info_value.set(value.val),
                None => {
                    self.status.insert(name, InfoValue::new(value.val));
                }
            }
        }
    }

    pub fn update_status(&mut self, dev: DeviceInfo) {
        self.merge_status_values(dev.general);
    }
}

impl TryFrom<ducoapi::DeviceInfo> for DucoBoxDevice {
    type Error = anyhow::Error;

    fn try_from(device_info: ducoapi::DeviceInfo) -> Result<Self> {
        let mut dev = DucoBoxDevice::new();
        dev.update_status(device_info);
        Ok(dev)
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_info_value() {
//         let mut val = InfoValue::new(StatusValue::String("foo".to_string()));
//         assert!(val.modified());
//         assert_eq!(val.get_and_reset(), StatusValue::String("foo".to_string()));
//         assert!(!val.modified());
//     }

//     #[test]
//     fn test_duco_node_action() {
//         let action = NodeActionDescription {
//             Action: "foo".to_string(),
//             ValType: "Enum".to_string(),
//             Enum: Some(vec!["bar".to_string()]),
//         };

//         let duco_action = DucoNodeAction::try_from(action).unwrap();
//         if let DucoNodeAction::SetEnum(action, values) = duco_action {
//             assert_eq!(action, "foo");
//             assert_eq!(values, vec!["bar".to_string()]);
//         } else {
//             panic!("Unexpected action type");
//         }
//     }

//     #[test]
//     fn test_ducobox_node() {
//         let node_info = NodeInfo {
//             Node: 1,
//             General: HashMap::from([
//                 ("Type".to_string(), StatusField::from("BOX")),
//                 ("SubType".to_string(), StatusField::from(1)),
//             ]),
//             Ventilation: HashMap::new(),
//             Sensor: None,
//         };

//         let mut node = DucoBoxNode::try_from(node_info).unwrap();
//         assert_eq!(node.number(), 1);

//         let mut topics = node.topics_that_need_updating();
//         topics.sort();
//         assert_eq!(
//             topics,
//             vec![
//                 MqttData::new("node_1/general/SubType", "1"),
//                 MqttData::new("node_1/general/Type", "BOX")
//             ]
//         );

//         let node_info_update = NodeInfo {
//             Node: 1,
//             General: HashMap::from([
//                 ("SubType".to_string(), StatusField::from(2)),
//                 ("Type".to_string(), StatusField::from("BOX")),
//             ]),
//             Ventilation: HashMap::new(),
//             Sensor: None,
//         };

//         node.update_status(node_info_update.clone()).unwrap();
//         assert_eq!(
//             node.topics_that_need_updating(),
//             vec![MqttData::new("node_1/general/SubType", "2"),]
//         );

//         node.update_status(node_info_update.clone()).unwrap();
//         assert!(node.topics_that_need_updating().is_empty(),);
//     }
// }
