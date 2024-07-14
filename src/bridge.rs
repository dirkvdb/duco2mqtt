use crate::ducoapi::{NodeAction, NodeInfo};
use crate::duxoboxnode::DucoBoxNode;
use crate::mqtt::{MqttConfig, MqttConnection, MqttData};
use crate::{ducoapi, Error, Result};
use std::path::PathBuf;
use tokio::time;

//const HASS_DISCOVERY_TOPIC: &str = "homeassistant";

fn http_client(ducobox_certificate: &Option<PathBuf>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder();
    if let Some(cert) = ducobox_certificate {
        builder = builder.add_root_certificate(reqwest::Certificate::from_pem(&std::fs::read(cert)?)?);
    } else {
        builder = builder.danger_accept_invalid_certs(true);
    }

    Ok(builder.build()?)
}

pub struct DucoMqttBridgeConfig {
    pub ducobox_address: String,
    pub ducobox_certificate: Option<PathBuf>,
    pub mqtt_config: MqttConfig,
    pub hass_discovery: bool,
    pub poll_interval: time::Duration,
}

pub struct DucoMqttBridge {
    mqtt: MqttConnection,
    ducobox_address: String,
    ducobox_certificate: Option<PathBuf>,
    poll_interval: time::Duration,
    nodes: Vec<DucoBoxNode>,
    mqtt_base_topic: String,
    hass_discovery: bool,
}

impl DucoMqttBridge {
    pub fn new(cfg: DucoMqttBridgeConfig) -> DucoMqttBridge {
        let mqtt_base_topic = format!("{}/", cfg.mqtt_config.base_topic);
        if cfg.ducobox_certificate.is_none() {
            log::warn!("No certificate provided, disabling certificate validation");
        }

        DucoMqttBridge {
            mqtt: MqttConnection::new(cfg.mqtt_config),
            ducobox_address: cfg.ducobox_address,
            ducobox_certificate: cfg.ducobox_certificate,
            poll_interval: cfg.poll_interval,
            nodes: Vec::new(),
            mqtt_base_topic,
            hass_discovery: cfg.hass_discovery,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut interval = time::interval(self.poll_interval);

        loop {
            tokio::select! {
                mqtt_msg = self.mqtt.poll() => {
                    if let Ok(Some(msg)) = mqtt_msg {
                        log::info!("MQTT cmnd: {} {}", msg.topic, msg.payload);
                        if let Err(err) = self.handle_node_command(msg).await {
                            log::error!("Failed to process command: {err}");
                        }
                    }
                }
                _ = interval.tick() => {

                    if let Err(err) = self.poll_ducobox().await {
                        log::error!("Failed to update duco status: {err}");
                        self.reset_nodes();
                        let _ = self.mqtt.publish_offline().await;
                    } else {
                        let _ = self.mqtt.publish_online().await;
                    }
                }
            }
        }
    }

    async fn discover_nodes(ducobox_address: &str, ducobox_certificate: &Option<PathBuf>) -> Result<Vec<DucoBoxNode>> {
        let client = http_client(ducobox_certificate)?;
        let nodes = ducoapi::get_nodes(&client, ducobox_address).await?;
        let node_actions = ducoapi::get_node_actions(&client, ducobox_address).await?;

        if nodes.len() != node_actions.len() {
            return Err(Error::Runtime(format!(
                "Node and action count mismatch ({} <-> {})",
                nodes.len(),
                node_actions.len()
            )));
        }

        let nodes = nodes
            .into_iter()
            .zip(node_actions.into_iter())
            .map(|(node_info, actions)| {
                if node_info.Node != actions.Node {
                    return Err(Error::Runtime(format!(
                        "Node mismatch ({} <-> {})",
                        node_info.Node, actions.Node
                    )));
                }

                let mut node = DucoBoxNode::try_from(node_info)?;
                node.set_actions(actions)?;

                Ok(node)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(nodes)
    }

    async fn poll_ducobox(&mut self) -> Result<()> {
        log::debug!("Update ducobox values");

        if self.nodes.is_empty() {
            self.nodes = DucoMqttBridge::discover_nodes(&self.ducobox_address, &self.ducobox_certificate).await?;

            if self.hass_discovery {
                for node in self.nodes.iter() {
                    if let Ok(mqtt_data) =
                        DucoMqttBridge::create_home_assistant_descriptions_for_node(node, self.mqtt_base_topic.as_str())
                    {
                        for md in mqtt_data {
                            self.mqtt.publish(md).await?;
                        }
                    }
                }
            }
        } else {
            let certificate = self.ducobox_certificate.clone();
            let client = http_client(&certificate)?;
            self.merge_nodes(ducoapi::get_nodes(&client, &self.ducobox_address).await?)?;
        }

        self.publish_nodes().await?;

        Ok(())
    }

    fn node_with_number(&mut self, nr: u16) -> Result<&mut DucoBoxNode> {
        if let Some(node) = self.nodes.iter_mut().find(|x| x.number() == nr) {
            Ok(node)
        } else {
            Err(Error::Runtime(format!("No node with id '{nr}'")))
        }
    }

    fn node_number_for_node_name(topic: &str) -> Result<u16> {
        let parts: Vec<&str> = topic.split('_').collect();
        if parts.len() == 3 && parts[0] == "duco" && parts[1] == "node" {
            return parts[2]
                .parse()
                .map_err(|_| Error::Runtime(format!("Invalid node number '{}'", parts[2])));
        }

        Err(Error::Runtime(format!("Invalid node topic provided: {}", topic)))
    }

    fn node_and_action_from_topic(topic: &str) -> Result<(u16, String)> {
        let topics: Vec<&str> = topic.split('/').collect();
        if topics.len() == 3 && topics[1] == "cmnd" {
            let node = DucoMqttBridge::node_number_for_node_name(topics[0])?;
            let action = String::from(topics[2]);

            return Ok((node, action));
        }

        Err(Error::Runtime(format!(
            "Invalid node topic provided: {} ({:?})",
            topic, topics
        )))
    }

    async fn handle_node_command(&mut self, msg: MqttData) -> Result<()> {
        if let Some(path) = msg.topic.strip_prefix(self.mqtt_base_topic.as_str()) {
            let (node_nr, action) = DucoMqttBridge::node_and_action_from_topic(path)?;

            let addr = self.ducobox_address.clone();
            let client = http_client(&self.ducobox_certificate)?;
            let node = self.node_with_number(node_nr)?;

            let action = NodeAction {
                Action: action,
                Val: msg.payload,
            };

            node.process_command(action, &client, &addr).await?;

            self.poll_ducobox().await?;
            return Ok(());
        }

        Err(Error::Runtime(format!("Unexpected command path: {}", msg.topic)))
    }

    fn merge_nodes(&mut self, new_nodes: Vec<NodeInfo>) -> Result<()> {
        for new_node in new_nodes {
            if let Ok(node) = self.node_with_number(new_node.Node) {
                node.update_status(new_node)?;
            } else {
                self.nodes.push(DucoBoxNode::try_from(new_node)?);
            }
        }

        Ok(())
    }

    async fn publish_nodes(&mut self) -> Result<()> {
        for node in self.nodes.iter_mut() {
            for mut mqtt_data in node.topics_that_need_updating() {
                mqtt_data.topic = format!("{}{}", self.mqtt_base_topic, mqtt_data.topic);
                log::info!("{}: {}", mqtt_data.topic, mqtt_data.payload);
                self.mqtt.publish(mqtt_data).await?;
            }
        }

        Ok(())
    }

    fn reset_nodes(&mut self) {
        for node in self.nodes.iter_mut() {
            node.reset();
        }
    }

    fn create_home_assistant_descriptions_for_node(_node: &DucoBoxNode, _base_topic: &str) -> Result<Vec<MqttData>> {
        let topics = Vec::new();
        // for register in DucoBoxNode::supported_holding_registers(node.node_type()) {
        //     match register {
        //         HoldingRegister::VentilationPosition => {
        //             let mut select = create_select_for_register::<VentilationPosition, HoldingRegister>(
        //                 node.number(),
        //                 base_topic,
        //                 register.to_string().as_str(),
        //                 register,
        //             );

        //             select.icon = Some(String::from("mdi:fan"));

        //             topics.push(MqttData {
        //                 topic: format!("{}/select/{}/config", HASS_DISCOVERY_TOPIC, select.unique_id),
        //                 payload: serde_json::to_string(&select)?,
        //             })
        //         }
        //         HoldingRegister::Identification => {
        //             let mut number =
        //                 create_number_for_register(node.number(), base_topic, register.to_string().as_str(), register);

        //             number.min = Some(0);
        //             number.max = Some(1);
        //             number.icon = Some(String::from("mdi:led-on"));

        //             topics.push(MqttData {
        //                 topic: format!("{}/number/{}/config", HASS_DISCOVERY_TOPIC, number.unique_id),
        //                 payload: serde_json::to_string(&number)?,
        //             })
        //         }
        //         HoldingRegister::SupplyTemperatureTargetZone1 | HoldingRegister::SupplyTemperatureTargetZone2 => {
        //             let mut sensor =
        //                 create_number_for_register(node.number(), base_topic, register.to_string().as_str(), register);

        //             sensor.device_class = Some(String::from("temperature"));
        //             sensor.min = Some(0);
        //             sensor.max = Some(400);
        //             sensor.unit_of_measurement = Some(String::from("0.1°C"));

        //             topics.push(MqttData {
        //                 topic: format!("{}/number/{}/config", HASS_DISCOVERY_TOPIC, sensor.unique_id),
        //                 payload: serde_json::to_string(&sensor)?,
        //             });

        //             log::debug!("{:?}", topics.last().unwrap());
        //         }
        //     }
        // }

        // for register in DucoBoxNode::supported_input_registers(node.node_type()) {
        //     let mut sensor =
        //         create_sensor_for_register(node.number(), base_topic, register.to_string().as_str(), register);
        //     sensor.state_class = Some(String::from("measurement"));

        //     match register {
        //         InputRegister::FlowRateVsTargetLevel => {
        //             sensor.unit_of_measurement = Some(String::from("%"));
        //             sensor.icon = Some(String::from("mdi:fan-clock"));
        //         }
        //         InputRegister::IndoorAirQualityBasedOnCO2 => {
        //             sensor.unit_of_measurement = Some(String::from("%"));
        //             sensor.icon = Some(String::from("mdi:molecule-co2"));
        //         }
        //         InputRegister::FilterTimeRemaining => {
        //             sensor.unit_of_measurement = Some(String::from("days"));
        //             sensor.icon = Some(String::from("mdi:calendar-clock"));
        //         }
        //         InputRegister::RemainingTimeCurrentVenilationMode => {
        //             sensor.unit_of_measurement = Some(String::from("seconds"));
        //             sensor.icon = Some(String::from("mdi:timer"));
        //         }
        //         _ => {
        //             continue;
        //         }
        //     }

        //     topics.push(MqttData {
        //         topic: format!("{}/sensor/{}/config", HASS_DISCOVERY_TOPIC, sensor.unique_id),
        //         payload: serde_json::to_string(&sensor)?,
        //     })
        // }

        Ok(topics)
    }
}

// Test for parsing node topics
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_number_for_node_name() {
        assert_eq!(DucoMqttBridge::node_number_for_node_name("duco_node_1").unwrap(), 1);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("duco_node_2").unwrap(), 2);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("duco_node_3").unwrap(), 3);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("duco_node_67").unwrap(), 67);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("duco_node_68").unwrap(), 68);
    }

    #[test]
    fn test_node_number_command_from_topic() {
        assert_eq!(
            DucoMqttBridge::node_and_action_from_topic("node_1/cmnd/SetVentilationState").unwrap(),
            (1, "SetVentilationState".to_string())
        );
        assert_eq!(
            DucoMqttBridge::node_and_action_from_topic("node_2/cmnd/SetIdentify").unwrap(),
            (2, "SetIdentify".to_string())
        );
    }
}
