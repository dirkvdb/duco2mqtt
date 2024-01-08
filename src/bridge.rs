use crate::duconodetypes::NodeType;
use crate::duxoboxnode::DucoBoxNode;
use crate::modbus::{DucoModbusConnection, ModbusConfig};
use crate::mqtt::{MqttConfig, MqttConnection};
use crate::MqttBridgeError;
use log;
use tokio::time;

pub struct DucoMqttBridgeConfig {
    pub modbus_config: ModbusConfig,
    pub mqtt_config: MqttConfig,
    pub mqtt_base_topic: String,
}

pub struct DucoMqttBridge {
    mqtt: MqttConnection,
    nodes: Vec<DucoBoxNode>,
    modbus_cfg: ModbusConfig,
    mqtt_base_topic: String,
}

impl DucoMqttBridge {
    pub fn new(cfg: DucoMqttBridgeConfig) -> DucoMqttBridge {
        DucoMqttBridge {
            mqtt: MqttConnection::new(cfg.mqtt_config),
            modbus_cfg: cfg.modbus_config,
            nodes: Vec::new(),
            mqtt_base_topic: cfg.mqtt_base_topic,
        }
    }

    pub async fn run(mut self) -> Result<(), MqttBridgeError> {
        let mut interval = time::interval(time::Duration::from_secs(60));

        loop {
            tokio::select! {
                mqtt_msg = self.mqtt.poll() => {
                    if let Ok(Some(msg)) = mqtt_msg {
                        log::debug!("Mqtt data received: {}", msg.topic);
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

    async fn update_nodes(
        &mut self,
        modbus: &mut DucoModbusConnection,
    ) -> Result<(), MqttBridgeError> {
        for node in self.nodes.iter_mut() {
            node.update_status(modbus).await?;
        }

        Ok(())
    }

    async fn publish_nodes(&mut self) -> Result<(), MqttBridgeError> {
        for node in self.nodes.iter_mut() {
            for mut mqtt_data in node.topics_that_need_updating() {
                mqtt_data.topic = format!("{}/{}", self.mqtt_base_topic, mqtt_data.topic);
                log::debug!("{}: {}", mqtt_data.topic, mqtt_data.payload);
                //First check topic names
                //self.mqtt.publish(mqtt_data).await?;
            }
        }

        Ok(())
    }

    fn reset_nodes(&mut self) {
        for node in self.nodes.iter_mut() {
            node.reset();
        }
    }

    async fn discover_nodes(
        &mut self,
        modbus: &mut DucoModbusConnection,
    ) -> Result<(), MqttBridgeError> {
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
                log::debug!("Node {} has node type {}", number, node_type);

                match node_type {
                    NodeType::DucoBox => {
                        self.nodes.push(DucoBoxNode::create_duco_box_node(number));
                    }
                    NodeType::Unknown => todo!(),
                    NodeType::RemoteControlRFBAT => todo!(),
                    NodeType::RemoteControlRFWired => todo!(),
                    NodeType::HumidityRoomSensor => todo!(),
                    NodeType::CO2RoomSensor => self
                        .nodes
                        .push(DucoBoxNode::create_co2_room_sensor_node(number)),
                    NodeType::SensorlessControlValve => self
                        .nodes
                        .push(DucoBoxNode::create_sensorless_control_valve(number)),
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
                log::debug!(
                    "Node {} has unsupported node type '{}', ignoring",
                    number,
                    node_type_nr
                );
            }
        }

        Ok(())
    }
}
