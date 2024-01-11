use crate::duconodetypes::{HoldingRegister, InputRegister, VentilationPosition};
use crate::duxoboxnode::DucoBoxNode;
use crate::hassdiscovery::{create_number_for_register, create_select_for_register, create_sensor_for_register};
use crate::modbus::{DucoModbusConnection, ModbusConfig};
use crate::mqtt::{MqttConfig, MqttConnection, MqttData};
use crate::MqttBridgeError;
use log;
use std::str::FromStr;
use tokio::time;

const HASS_DISCOVERY_TOPIC: &str = "homeassistant";

pub struct DucoMqttBridgeConfig {
    pub modbus_config: ModbusConfig,
    pub mqtt_config: MqttConfig,
    pub hass_discovery: bool,
}

pub struct DucoMqttBridge {
    mqtt: MqttConnection,
    nodes: Vec<DucoBoxNode>,
    modbus_cfg: ModbusConfig,
    mqtt_base_topic: String,
    hass_discovery: bool,
}

impl DucoMqttBridge {
    pub fn new(cfg: DucoMqttBridgeConfig) -> DucoMqttBridge {
        let mqtt_base_topic = format!("{}/", cfg.mqtt_config.base_topic);
        DucoMqttBridge {
            mqtt: MqttConnection::new(cfg.mqtt_config),
            modbus_cfg: cfg.modbus_config,
            nodes: Vec::new(),
            mqtt_base_topic,
            hass_discovery: cfg.hass_discovery,
        }
    }

    pub async fn run(mut self) -> Result<(), MqttBridgeError> {
        let mut interval = time::interval(self.modbus_cfg.poll_interval);

        loop {
            tokio::select! {
                mqtt_msg = self.mqtt.poll() => {
                    if let Ok(Some(msg)) = mqtt_msg {
                        log::debug!("MQTT cmnd: {} {}", msg.topic, msg.payload);
                        if let Err(err) = self.handle_node_command(&msg).await {
                            log::error!("Failed to process command: {}: {} ({err})", msg.topic, msg.payload);
                        }
                    }
                }
                _ = interval.tick() => {
                    if let Err(err) = self.poll_modbus().await {
                        log::error!("Failed to update status: {err}");
                        self.reset_nodes();
                        let _ = self.publish_nodes().await;
                        let _ = self.mqtt.publish_offline().await;
                    } else {
                        let _ = self.mqtt.publish_online().await;
                    }
                }
            }
        }
    }

    async fn poll_modbus(&mut self) -> Result<(), MqttBridgeError> {
        log::debug!("Update modbus values");
        let mut modbus = DucoModbusConnection::new();
        modbus.connect(&self.modbus_cfg).await?;

        if self.nodes.is_empty() {
            self.discover_nodes(&mut modbus).await?;
            if self.hass_discovery {
                for node in self.nodes.iter() {
                    if let Ok(mqtt_data) = DucoMqttBridge::create_home_assistant_descriptions_for_node(
                        &node,
                        &self.mqtt_base_topic.as_str(),
                    ) {
                        for md in mqtt_data {
                            self.mqtt.publish(md).await?;
                        }
                    }
                }
            }
        }

        self.update_nodes(&mut modbus).await?;
        self.publish_nodes().await?;

        Ok(())
    }

    fn node_with_number(&mut self, nr: u16) -> Result<&mut DucoBoxNode, MqttBridgeError> {
        if let Some(node) = self.nodes.iter_mut().find(|x| x.number() == nr) {
            Ok(node)
        } else {
            Err(MqttBridgeError::RuntimeError(format!("No node with id '{nr}'")))
        }
    }

    fn node_number_for_node_name(topic: &str) -> Result<u16, MqttBridgeError> {
        let parts: Vec<&str> = topic.split('_').collect();
        if parts.len() == 2 && parts[0] == "node" {
            return parts[1]
                .parse()
                .map_err(|_| MqttBridgeError::RuntimeError(format!("Invalid node number '{}'", parts[1])));
        }

        Err(MqttBridgeError::RuntimeError(format!(
            "Invalid node topic provided: {}",
            topic
        )))
    }

    fn node_number_cmd_from_topic(topic: &str) -> Result<(u16, HoldingRegister), MqttBridgeError> {
        let topics: Vec<&str> = topic.split('/').collect();
        if topics.len() == 3 && topics[1] == "cmnd" {
            let node = DucoMqttBridge::node_number_for_node_name(topics[0])?;
            let holding_register = HoldingRegister::from_str(topics[2])
                .map_err(|_| MqttBridgeError::RuntimeError(format!("Invalid command : '{}'", topics[0])))?;

            return Ok((node, holding_register));
        }

        Err(MqttBridgeError::RuntimeError(format!(
            "Invalid node topic provided: {}",
            topic
        )))
    }

    async fn handle_node_command(&mut self, msg: &MqttData) -> Result<(), MqttBridgeError> {
        if let Some(path) = msg.topic.strip_prefix(self.mqtt_base_topic.as_str()) {
            let (node_nr, reg) = DucoMqttBridge::node_number_cmd_from_topic(path)?;

            let mut modbus = DucoModbusConnection::new();
            modbus.connect(&self.modbus_cfg).await?;
            let node = self.node_with_number(node_nr)?;
            node.process_command(reg, msg.payload.as_str(), &mut modbus).await?;
            self.update_nodes(&mut modbus).await?;

            return Ok(());
        }

        Err(MqttBridgeError::RuntimeError(format!(
            "Unexpected command path: {}",
            msg.topic
        )))
    }

    async fn update_nodes(&mut self, modbus: &mut DucoModbusConnection) -> Result<(), MqttBridgeError> {
        for node in self.nodes.iter_mut() {
            node.update_status(modbus).await?;
        }

        Ok(())
    }

    async fn publish_nodes(&mut self) -> Result<(), MqttBridgeError> {
        for node in self.nodes.iter_mut() {
            for mut mqtt_data in node.topics_that_need_updating() {
                mqtt_data.topic = format!("{}{}", self.mqtt_base_topic, mqtt_data.topic);
                log::debug!("{}: {}", mqtt_data.topic, mqtt_data.payload);
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

    fn create_home_assistant_descriptions_for_node(
        node: &DucoBoxNode,
        base_topic: &str,
    ) -> Result<Vec<MqttData>, MqttBridgeError> {
        let mut topics = Vec::new();
        for register in DucoBoxNode::supported_holding_registers(node.node_type()) {
            match register {
                HoldingRegister::VentilationPosition => {
                    let mut select = create_select_for_register::<VentilationPosition, HoldingRegister>(
                        node.number(),
                        base_topic,
                        register.to_string().as_str(),
                        register,
                    );

                    select.icon = Some(String::from("mdi:fan"));

                    topics.push(MqttData {
                        topic: format!("{}/select/{}/config", HASS_DISCOVERY_TOPIC, select.unique_id),
                        payload: serde_json::to_string(&select)?,
                    })
                }
                HoldingRegister::Identification => {
                    let mut number =
                        create_number_for_register(node.number(), base_topic, register.to_string().as_str(), register);

                    number.min = Some(0);
                    number.max = Some(1);
                    number.icon = Some(String::from("mdi:led-on"));

                    topics.push(MqttData {
                        topic: format!("{}/number/{}/config", HASS_DISCOVERY_TOPIC, number.unique_id),
                        payload: serde_json::to_string(&number)?,
                    })
                }
                HoldingRegister::SupplyTemperatureTargetZone1 | HoldingRegister::SupplyTemperatureTargetZone2 => {
                    let mut sensor =
                        create_number_for_register(node.number(), base_topic, register.to_string().as_str(), register);

                    sensor.device_class = Some(String::from("temperature"));
                    sensor.min = Some(0);
                    sensor.max = Some(400);
                    sensor.unit_of_measurement = Some(String::from("0.1°C"));

                    topics.push(MqttData {
                        topic: format!("{}/number/{}/config", HASS_DISCOVERY_TOPIC, sensor.unique_id),
                        payload: serde_json::to_string(&sensor)?,
                    });

                    log::debug!("{:?}", topics.last().unwrap());
                }
            }
        }

        for register in DucoBoxNode::supported_input_registers(node.node_type()) {
            let mut sensor =
                create_sensor_for_register(node.number(), base_topic, register.to_string().as_str(), register);
            sensor.state_class = Some(String::from("measurement"));

            match register {
                InputRegister::FlowRateVsTargetLevel => {
                    sensor.unit_of_measurement = Some(String::from("%"));
                    sensor.icon = Some(String::from("mdi:fan-clock"));
                }
                InputRegister::IndoorAirQualityBasedOnCO2 => {
                    sensor.unit_of_measurement = Some(String::from("%"));
                    sensor.icon = Some(String::from("mdi:molecule-co2"));
                }
                InputRegister::FilterTimeRemaining => {
                    sensor.unit_of_measurement = Some(String::from("days"));
                    sensor.icon = Some(String::from("mdi:calendar-clock"));
                }
                _ => {
                    continue;
                }
            }

            topics.push(MqttData {
                topic: format!("{}/sensor/{}/config", HASS_DISCOVERY_TOPIC, sensor.unique_id),
                payload: serde_json::to_string(&sensor)?,
            })
        }

        Ok(topics)
    }

    async fn discover_nodes(&mut self, modbus: &mut DucoModbusConnection) -> Result<(), MqttBridgeError> {
        self.nodes.clear();

        let mut node_indexes: Vec<u16> = Vec::new();

        let regs = modbus.read_input_registers(0, 9).await?;
        let mut index = 0;
        for reg in regs {
            for i in 0..16 {
                // Check if the bit at position i is 1
                if (reg & (1 << i)) != 0 {
                    node_indexes.push((index * 16) + i);
                }
            }

            index += 1;
        }

        for number in node_indexes {
            let node_type_nr = modbus.read_input_register(number * 100).await?;
            if let Some(node_type) = num::FromPrimitive::from_u16(node_type_nr) {
                if let Some(node) = DucoBoxNode::create_for_node_type(node_type, number) {
                    self.nodes.push(node);
                } else {
                    log::warn!("Node {node_type} support not implemented");
                }
            }
        }

        Ok(())
    }
}

// Test for parsing node topics
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_number_for_node_name() {
        assert_eq!(DucoMqttBridge::node_number_for_node_name("node_1").unwrap(), 1);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("node_2").unwrap(), 2);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("node_3").unwrap(), 3);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("node_67").unwrap(), 67);
        assert_eq!(DucoMqttBridge::node_number_for_node_name("node_68").unwrap(), 68);
    }

    #[test]
    fn test_node_number_command_from_topic() {
        assert_eq!(
            DucoMqttBridge::node_number_cmd_from_topic("node_1/cmnd/ventilation_position").unwrap(),
            (1, HoldingRegister::VentilationPosition)
        );
        assert_eq!(
            DucoMqttBridge::node_number_cmd_from_topic("node_2/cmnd/identification").unwrap(),
            (2, HoldingRegister::Identification)
        );
        assert_eq!(
            DucoMqttBridge::node_number_cmd_from_topic("node_2/cmnd/supply_temperature_target_zone1").unwrap(),
            (2, HoldingRegister::SupplyTemperatureTargetZone1)
        );
        assert_eq!(
            DucoMqttBridge::node_number_cmd_from_topic("node_2/cmnd/supply_temperature_target_zone2").unwrap(),
            (2, HoldingRegister::SupplyTemperatureTargetZone2)
        );
    }
}
