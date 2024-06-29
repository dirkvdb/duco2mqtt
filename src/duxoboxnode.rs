use std::str::FromStr;

use crate::{
    duconodetypes::{HoldingRegister, InputRegister, NodeType, VentilationPosition},
    modbus::DucoModbusConnection,
    mqtt::MqttData,
    Error,
};

pub const UNKNOWN: &str = "UNKNOWN";

fn optional_enum_string<T: std::string::ToString + num::FromPrimitive>(val: u16) -> String {
    let enum_type: Option<T> = num::FromPrimitive::from_u16(val);
    match enum_type {
        Some(enum_type) => format!("{}", enum_type.to_string()),
        None => String::from(UNKNOWN),
    }
}

enum Register {
    Holding(HoldingRegister, RegisterValue),
    Input(InputRegister, RegisterValue),
}

struct RegisterValue {
    value: Option<u16>,
    modified: bool,
}

impl RegisterValue {
    fn new() -> RegisterValue {
        RegisterValue {
            value: None,
            modified: true,
        }
    }

    fn set(&mut self, val: Option<u16>) {
        if val != self.value {
            self.modified = true;
            self.value = val;
        }
    }

    fn clear(&mut self) {
        self.set(None);
    }

    fn modified(&self) -> bool {
        self.modified
    }

    fn get_and_reset(&mut self) -> Option<u16> {
        self.modified = false;
        self.value
    }
}

pub struct DucoBoxNode {
    number: u16,
    node_type: NodeType,
    registers: Vec<Register>,
}

impl DucoBoxNode {
    pub fn create_for_node_type(node_type: NodeType, number: u16) -> Option<DucoBoxNode> {
        let mut registers = Vec::new();
        for input_register in DucoBoxNode::supported_input_registers(node_type) {
            registers.push(Register::Input(input_register, RegisterValue::new()));
        }

        for holding_register in DucoBoxNode::supported_holding_registers(node_type) {
            registers.push(Register::Holding(holding_register, RegisterValue::new()));
        }

        if registers.is_empty() {
            None
        } else {
            Some(DucoBoxNode {
                number,
                node_type,
                registers,
            })
        }
    }

    pub fn number(&self) -> u16 {
        self.number
    }

    pub fn node_type(&self) -> NodeType {
        self.node_type
    }

    pub fn supported_input_registers(node_type: NodeType) -> Vec<InputRegister> {
        match node_type {
            NodeType::DucoBox => vec![
                InputRegister::SystemType,
                InputRegister::FlowRateVsTargetLevel,
                InputRegister::FilterTimeRemaining,
            ],
            NodeType::CO2RoomSensor => vec![
                InputRegister::SystemType,
                InputRegister::RemainingTimeCurrentVenilationMode,
                InputRegister::IndoorAirQualityBasedOnCO2,
            ],
            NodeType::SensorlessControlValve => vec![
                InputRegister::SystemType,
                InputRegister::RemainingTimeCurrentVenilationMode,
                InputRegister::FlowRateVsTargetLevel,
            ],
            _ => Vec::new(),
        }
    }

    pub fn supported_holding_registers(node_type: NodeType) -> Vec<HoldingRegister> {
        match node_type {
            NodeType::DucoBox => vec![
                HoldingRegister::VentilationPosition,
                HoldingRegister::SupplyTemperatureTargetZone1,
                HoldingRegister::SupplyTemperatureTargetZone2,
            ],
            NodeType::CO2RoomSensor => vec![HoldingRegister::VentilationPosition, HoldingRegister::Identification],
            NodeType::SensorlessControlValve => vec![HoldingRegister::VentilationPosition],
            _ => Vec::new(),
        }
    }

    pub fn reset(&mut self) {
        for register in self.registers.iter_mut() {
            match register {
                Register::Holding(_reg, val) => {
                    val.clear();
                }
                Register::Input(_reg, val) => {
                    val.clear();
                }
            }
        }
    }

    pub async fn process_command(
        &self,
        reg: HoldingRegister,
        value: &str,
        modbus: &mut DucoModbusConnection,
    ) -> Result<(), Error> {
        let register_value;
        match reg {
            HoldingRegister::VentilationPosition => {
                register_value = VentilationPosition::from_str(value).unwrap() as u16;
            }
            HoldingRegister::Identification => {
                register_value = value.parse::<u16>().map_err(|_| {
                    Error::Runtime(format!("Identifcation should be an integral value, got {}", value,))
                })?;

                if register_value > 1 {
                    return Err(Error::Runtime(format!(
                        "Identifcation should be 0 or 1, got '{}'",
                        register_value,
                    )));
                }
            }
            HoldingRegister::SupplyTemperatureTargetZone1 => todo!(),
            HoldingRegister::SupplyTemperatureTargetZone2 => todo!(),
        }

        let address = self.number() * 100 + reg as u16;
        log::info!("Write register address {address} with value: {register_value}");
        modbus.write_holding_register(address, register_value).await?;

        Ok(())
    }

    pub fn topics_that_need_updating(&mut self) -> Vec<MqttData> {
        let mut topics = Vec::new();
        for register in self.registers.iter_mut() {
            match register {
                Register::Holding(reg, val) => {
                    if val.modified() {
                        let val = val.get_and_reset();
                        topics.push(MqttData {
                            topic: DucoBoxNode::register_topic(self.number, *reg),
                            payload: DucoBoxNode::holding_register_value_string(*reg, val),
                        });
                    }
                }
                Register::Input(reg, val) => {
                    if val.modified() {
                        let val = val.get_and_reset();
                        topics.push(MqttData {
                            topic: DucoBoxNode::register_topic(self.number, *reg),
                            payload: DucoBoxNode::input_register_value_string(*reg, val),
                        });
                    }
                }
            }
        }

        topics
    }

    pub async fn update_status(&mut self, modbus: &mut DucoModbusConnection) -> Result<(), Error> {
        for register in self.registers.iter_mut() {
            match register {
                Register::Input(reg, val) => {
                    let addr = self.number * 100 + *reg as u16;
                    let mut register_val = modbus.read_input_register(addr).await;
                    if let Err(err) = register_val.as_ref() {
                        if reg == &InputRegister::RemainingTimeCurrentVenilationMode {
                            // This register can only be read when the ventilation is running in manual mode
                            register_val = Ok(0);
                        } else {
                            log::warn!("Failed to read input register: {err}")
                        }
                    }
                    val.set(register_val.ok());
                }
                Register::Holding(reg, val) => {
                    let addr = self.number * 100 + *reg as u16;
                    let register_val = modbus.read_holding_register(addr).await;
                    if let Err(err) = register_val.as_ref() {
                        log::warn!("Failed to read holding register: {err}")
                    }
                    val.set(register_val.ok());
                }
            }
        }

        Ok(())
    }

    fn input_register_value_string(reg: InputRegister, val: Option<u16>) -> String {
        match val {
            Some(val) => match reg {
                InputRegister::SystemType => optional_enum_string::<NodeType>(val),
                InputRegister::FlowRateVsTargetLevel
                | InputRegister::IndoorAirQualityBasedOnCO2
                | InputRegister::RemainingTimeCurrentVenilationMode
                | InputRegister::FilterTimeRemaining => {
                    format!("{val}")
                }
            },
            None => String::from(UNKNOWN),
        }
    }

    fn register_topic<T: std::string::ToString>(node_number: u16, reg: T) -> String {
        format!("node_{}/{}", node_number, reg.to_string())
    }

    fn holding_register_value_string(reg: HoldingRegister, val: Option<u16>) -> String {
        match val {
            Some(val) => match reg {
                HoldingRegister::VentilationPosition => optional_enum_string::<VentilationPosition>(val),
                HoldingRegister::Identification
                | HoldingRegister::SupplyTemperatureTargetZone1
                | HoldingRegister::SupplyTemperatureTargetZone2 => {
                    format!("{val}")
                }
            },
            None => String::from(UNKNOWN),
        }
    }
}
