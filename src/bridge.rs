use crate::duconode::DucoNode;
use crate::duconode::NodeType;
use crate::modbus::{DucoModbusConnection, ModbusConfig};
use crate::mqtt::{MqttConfig, MqttConnection};
use crate::nodetypes::{CO2RoomSensorNode, DucoBoxNode, SensorlessControlValveNode};
use crate::MqttBridgeError;
use log;
use tokio::time;

pub struct DucoMqttBridgeConfig {
    pub modbus_config: ModbusConfig,
    pub mqtt_config: MqttConfig,
}

enum Node {
    DucoBoxNode(DucoBoxNode),
    CO2RoomSensorNode(CO2RoomSensorNode),
    SensorlessControlValveNode(SensorlessControlValveNode),
}

pub struct DucoMqttBridge {
    mqtt: MqttConnection,
    modbus: DucoModbusConnection,
    nodes: Vec<Node>,
}

impl DucoMqttBridge {
    pub fn new(cfg: DucoMqttBridgeConfig) -> DucoMqttBridge {
        DucoMqttBridge {
            mqtt: MqttConnection::new(cfg.mqtt_config),
            modbus: DucoModbusConnection::new(cfg.modbus_config),
            nodes: Vec::new(),
        }
    }

    pub async fn run(mut self) -> Result<(), MqttBridgeError> {
        let mut interval = time::interval(time::Duration::from_secs(5));

        loop {
            tokio::select! {
                mqtt_msg = self.mqtt.poll() => {
                    if let Ok(Some(msg)) = mqtt_msg {
                        log::debug!("Mqtt data received: {}", msg.topic);
                    }
                }
                _ = interval.tick() => {
                    log::debug!("Update modbus values");
                    if !self.modbus.is_connected() {
                        log::debug!("Modbus not connected: trying to reconnect");
                        if let Err(err) = self.modbus.connect().await {
                            log::warn!("Failed to establish modbus connection: {err}");
                        } else {
                            log::debug!("Modbus connection restored");
                            if let Err(err) = self.discover_nodes().await {
                                log::error!("Failed to populate node information: {err}");
                            }
                        }
                    }

                    if self.modbus.is_connected() {
                        if let Err(err) = self.update_nodes().await {
                            log::warn!("Failed to update node states: {err}")
                        }
                    }
                }
            }
        }
    }

    async fn update_nodes(&mut self) -> Result<(), MqttBridgeError> {
        for node in self.nodes.iter_mut() {
            match node {
                Node::DucoBoxNode(node) => {
                    node.update_status(&mut self.modbus).await?;
                    log::debug!("Node: {:?}", node);
                }
                Node::CO2RoomSensorNode(node) => {
                    node.update_status(&mut self.modbus).await?;
                    log::debug!("Node: {:?}", node);
                }
                Node::SensorlessControlValveNode(node) => {
                    node.update_status(&mut self.modbus).await?;
                    log::debug!("Node: {:?}", node);
                }
            }
        }

        Ok(())
    }

    async fn discover_nodes(&mut self) -> Result<(), MqttBridgeError> {
        self.nodes.clear();

        let mut node_indexes: Vec<u16> = Vec::new();

        let regs = self.modbus.read_input_registers(0, 9).await?;
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

        for node in node_indexes {
            let node_type_nr = self.modbus.read_input_register(node * 100).await?;
            if let Some(node_type) = num::FromPrimitive::from_u16(node_type_nr) {
                log::debug!("Node {} has node type {}", node, node_type);

                match node_type {
                    NodeType::DucoBox => {
                        self.nodes.push(Node::DucoBoxNode(DucoBoxNode::new(node)));
                    }
                    NodeType::Unknown => todo!(),
                    NodeType::RemoteControlRFBAT => todo!(),
                    NodeType::RemoteControlRFWired => todo!(),
                    NodeType::HumidityRoomSensor => todo!(),
                    NodeType::CO2RoomSensor => self.nodes.push(Node::CO2RoomSensorNode(CO2RoomSensorNode::new(node))),
                    NodeType::SensorlessControlValve => {
                        self.nodes
                            .push(Node::SensorlessControlValveNode(SensorlessControlValveNode::new(node)));
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
                log::debug!("Node {} has unsupported node type '{}', ignoring", node, node_type_nr);
            }
        }

        Ok(())
    }
}
