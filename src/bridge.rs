use crate::duconodetypes::{HoldingRegister, NodeType};
use crate::duxoboxnode::DucoBoxNode;
use crate::modbus::{DucoModbusConnection, ModbusConfig};
use crate::mqtt::{MqttConfig, MqttConnection, MqttData};
use crate::MqttBridgeError;
use log;
use std::str::FromStr;
use tokio::time;

pub struct DucoMqttBridgeConfig {
    pub modbus_config: ModbusConfig,
    pub mqtt_config: MqttConfig,
}

pub struct DucoMqttBridge {
    mqtt: MqttConnection,
    nodes: Vec<DucoBoxNode>,
    modbus_cfg: ModbusConfig,
    mqtt_base_topic: String,
}

impl DucoMqttBridge {
    pub fn new(cfg: DucoMqttBridgeConfig) -> DucoMqttBridge {
        let mqtt_base_topic = format!("{}/", cfg.mqtt_config.base_topic);
        DucoMqttBridge {
            mqtt: MqttConnection::new(cfg.mqtt_config),
            modbus_cfg: cfg.modbus_config,
            nodes: Vec::new(),
            mqtt_base_topic,
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
            node.update_status(&mut modbus).await?;

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
                match node_type {
                    NodeType::DucoBox => {
                        self.nodes.push(DucoBoxNode::create_duco_box_node(number));
                    }
                    NodeType::Unknown => todo!(),
                    NodeType::RemoteControlRFBAT => todo!(),
                    NodeType::RemoteControlRFWired => todo!(),
                    NodeType::HumidityRoomSensor => todo!(),
                    NodeType::CO2RoomSensor => self.nodes.push(DucoBoxNode::create_co2_room_sensor_node(number)),
                    NodeType::SensorlessControlValve => {
                        self.nodes.push(DucoBoxNode::create_sensorless_control_valve(number))
                    }
                    NodeType::HumidityControlValve => todo!(),
                    NodeType::CO2ControlValve => todo!(),
                    NodeType::SwitchSensor => todo!(),
                    NodeType::ControlUnit => todo!(),
                    NodeType::CO2RHControlValve => todo!(),
                    NodeType::RemoteControlSunControlRFWired => todo!(),
                    NodeType::RemoteControlNightventRFWired => todo!(),
                    NodeType::ExternalMultiZoneValve => todo!(),
                    NodeType::HumidityBoxSensor => todo!(),
                    NodeType::CO2BoxSensors => todo!(),
                    NodeType::DucoWeatherStation => todo!(),
                }
            } else {
                log::debug!("Node {} has unsupported node type '{}', ignoring", number, node_type_nr);
            }
        }

        Ok(())
    }
}
