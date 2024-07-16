use crate::ducoapi::NodeInfo;
use crate::ducoboxdevice::DucoBoxDevice;
use crate::ducoboxnode::DucoBoxNode;
use crate::hassdiscovery::{self};
use crate::mqtt::{MqttConfig, MqttConnection, MqttData};
use crate::{ducoapi, Result};
use anyhow::{anyhow, ensure};
use std::net::SocketAddr;
use std::path::PathBuf;
use tokio::time;

pub struct DucoMqttBridgeConfig {
    pub ducobox_host: String,
    pub ducobox_ip_address: Option<String>,
    pub ducobox_certificate: Option<PathBuf>,
    pub mqtt_config: MqttConfig,
    pub hass_discovery: bool,
    pub poll_interval: time::Duration,
}

pub struct DucoMqttBridge {
    mqtt: MqttConnection,
    ducobox_host: String,
    ducobox_ip_address: Option<SocketAddr>,
    ducobox_certificate: Option<PathBuf>,
    poll_interval: time::Duration,
    device_info: Option<DucoBoxDevice>,
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

        let ip_addr = cfg
            .ducobox_ip_address
            .map(|ip| format!("{}:443", ip).parse().expect("Invalid ip address"));

        DucoMqttBridge {
            mqtt: MqttConnection::new(cfg.mqtt_config),
            ducobox_host: cfg.ducobox_host,
            ducobox_ip_address: ip_addr,
            ducobox_certificate: cfg.ducobox_certificate,
            poll_interval: cfg.poll_interval,
            device_info: None,
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
                            log::error!("Failed to process command: {:#}", err);
                        }
                    }
                }
                _ = interval.tick() => {
                    let client = self.http_client()?;
                    if let Err(err) = self.poll_ducobox(&client).await {
                        log::error!("Failed to update duco status: {:#}", err);
                        self.reset_status();
                        let _ = self.mqtt.publish_offline().await;
                    } else {
                        let _ = self.mqtt.publish_online().await;
                    }
                }
            }
        }
    }

    async fn discover_nodes(ducobox_address: &str, client: &reqwest::Client) -> Result<Vec<DucoBoxNode>> {
        let nodes = ducoapi::get_nodes(client, ducobox_address).await?;
        let node_actions = ducoapi::get_node_actions(client, ducobox_address).await?;

        ensure!(
            nodes.len() == node_actions.len(),
            "Node and action count mismatch ({} <-> {})",
            nodes.len(),
            node_actions.len()
        );

        let nodes = nodes
            .into_iter()
            .zip(node_actions.into_iter())
            .map(|(node_info, actions)| {
                ensure!(
                    node_info.Node == actions.Node,
                    "Node mismatch ({} <-> {})",
                    node_info.Node,
                    actions.Node
                );

                let mut node = DucoBoxNode::try_from(node_info)?;
                node.set_actions(actions)?;

                Ok(node)
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(nodes)
    }

    async fn poll_ducobox(&mut self, client: &reqwest::Client) -> Result<()> {
        log::debug!("Update ducobox values");

        let dev = DucoBoxDevice::try_from(ducoapi::get_device_info(client, &self.ducobox_host).await?)?;

        if self.device_info.is_none() && self.hass_discovery {
            if let Ok(mqtt_data) = DucoMqttBridge::create_hass_descriptions_for_device(&self.mqtt_base_topic) {
                self.mqtt.publish_multiple(mqtt_data).await?;
            }
        }

        self.device_info = Some(dev);

        if self.nodes.is_empty() {
            self.nodes = DucoMqttBridge::discover_nodes(&self.ducobox_host, client).await?;

            if self.hass_discovery {
                for node in &self.nodes {
                    match DucoMqttBridge::create_hass_descriptions_for_node(node, &self.mqtt_base_topic) {
                        Ok(mqtt_data) => {
                            self.mqtt.publish_multiple(mqtt_data).await?;
                        }
                        Err(err) => {
                            log::error!("Failed to create home assistant descriptions: {:#}", err);
                        }
                    }
                }
            }
        } else {
            self.merge_nodes(ducoapi::get_nodes(client, &self.ducobox_host).await?)?;
        }

        self.publish_device_info().await?;
        self.publish_nodes().await?;

        Ok(())
    }

    fn node_with_number(&mut self, nr: u16) -> Result<&mut DucoBoxNode> {
        if let Some(node) = self.nodes.iter_mut().find(|x| x.number() == nr) {
            Ok(node)
        } else {
            Err(anyhow!("No node with id '{nr}'"))
        }
    }

    fn node_number_for_node_name(topic: &str) -> Result<u16> {
        let parts: Vec<&str> = topic.split('_').collect();
        if parts.len() == 3 && parts[0] == "duco" && parts[1] == "node" {
            return parts[2]
                .parse()
                .map_err(|_| anyhow!("Invalid node number '{}'", parts[2]));
        }

        Err(anyhow!("Invalid node topic provided: {}", topic))
    }

    fn node_and_action_from_topic(topic: &str) -> Result<(u16, String)> {
        let topics: Vec<&str> = topic.split('/').collect();
        if topics.len() == 3 && topics[1] == "cmnd" {
            let node = DucoMqttBridge::node_number_for_node_name(topics[0])?;
            let action = String::from(topics[2]);

            return Ok((node, action));
        }

        Err(anyhow!("Invalid node topic provided: {} ({:?})", topic, topics))
    }

    async fn handle_node_command(&mut self, msg: MqttData) -> Result<()> {
        if let Some(path) = msg.topic.strip_prefix(self.mqtt_base_topic.as_str()) {
            let (node_nr, action_name) = DucoMqttBridge::node_and_action_from_topic(path)?;

            let addr = self.ducobox_host.clone();
            let client = self.http_client()?;
            let node = self.node_with_number(node_nr)?;

            node.process_command(action_name, msg.payload, &client, &addr).await?;

            self.poll_ducobox(&client).await?;
            return Ok(());
        }

        Err(anyhow!("Unexpected command path: {}", msg.topic))
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

    async fn publish_device_info(&mut self) -> Result<()> {
        if let Some(ref mut device_info) = &mut self.device_info {
            for mut mqtt_data in device_info.topics_that_need_updating() {
                mqtt_data.topic = format!("{}{}", self.mqtt_base_topic, mqtt_data.topic);
                log::info!("{}: {}", mqtt_data.topic, mqtt_data.payload);
                self.mqtt.publish(mqtt_data).await?;
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

    fn reset_status(&mut self) {
        if let Some(ref mut device_info) = &mut self.device_info {
            device_info.reset();
        }
        for node in self.nodes.iter_mut() {
            node.reset();
        }
    }

    fn create_hass_descriptions_for_device(base_topic: &str) -> Result<Vec<MqttData>> {
        Ok(vec![hassdiscovery::filter_days_remaining_topic(base_topic)?])
    }

    fn create_hass_descriptions_for_node(node: &DucoBoxNode, base_topic: &str) -> Result<Vec<MqttData>> {
        let mut topics = Vec::new();

        match node.node_type() {
            crate::duconodetypes::NodeType::DucoBox | crate::duconodetypes::NodeType::CO2ControlValve => {
                topics.push(hassdiscovery::ventilation_state_topic(
                    node,
                    base_topic,
                    node.valid_action_values("SetVentilationState")?,
                )?);
                topics.push(hassdiscovery::flow_level_target_topic(node, base_topic)?);
                topics.push(hassdiscovery::state_time_remaining_topic(node, base_topic)?);
                topics.push(hassdiscovery::identify_topic(node, base_topic)?);
            }
            crate::duconodetypes::NodeType::CO2RoomSensor => {
                topics.push(hassdiscovery::co2_sensor_topic(node, base_topic)?);
                topics.push(hassdiscovery::identify_topic(node, base_topic)?);
            }
            crate::duconodetypes::NodeType::RemoteControlRFBAT => todo!(),
            crate::duconodetypes::NodeType::RemoteControlRFWired => todo!(),
            crate::duconodetypes::NodeType::HumidityRoomSensor => todo!(),
            crate::duconodetypes::NodeType::SensorlessControlValve => todo!(),
            crate::duconodetypes::NodeType::HumidityControlValve => todo!(),
            crate::duconodetypes::NodeType::SwitchSensor => todo!(),
            crate::duconodetypes::NodeType::ControlUnit => todo!(),
            crate::duconodetypes::NodeType::CO2RHControlValve => todo!(),
            crate::duconodetypes::NodeType::RemoteControlSunControlRFWired => todo!(),
            crate::duconodetypes::NodeType::RemoteControlNightventRFWired => todo!(),
            crate::duconodetypes::NodeType::ExternalMultiZoneValve => todo!(),
            crate::duconodetypes::NodeType::HumidityBoxSensor => todo!(),
            crate::duconodetypes::NodeType::CO2BoxSensors => todo!(),
            crate::duconodetypes::NodeType::DucoWeatherStation => todo!(),
            crate::duconodetypes::NodeType::Unknown => {}
        }

        // for register in DucoBoxNode::supported_holding_registers(node.node_type()) {
        //     match register {
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

        Ok(topics)
    }

    fn http_client(&self) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder().connect_timeout(time::Duration::from_secs(15));

        if let Some(addr) = self.ducobox_ip_address {
            builder = builder.resolve(&self.ducobox_host, addr);
        }

        if let Some(ref cert) = self.ducobox_certificate {
            builder = builder.use_rustls_tls();
            for cert in reqwest::Certificate::from_pem_bundle(&std::fs::read(cert)?)? {
                builder = builder.add_root_certificate(cert);
            }
        } else {
            builder = builder.danger_accept_invalid_certs(true);
        }

        Ok(builder.build()?)
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
            DucoMqttBridge::node_and_action_from_topic("duco_node_1/cmnd/SetVentilationState").unwrap(),
            (1, "SetVentilationState".to_string())
        );
        assert_eq!(
            DucoMqttBridge::node_and_action_from_topic("duco_node_2/cmnd/SetIdentify").unwrap(),
            (2, "SetIdentify".to_string())
        );
    }
}
