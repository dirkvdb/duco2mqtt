use std::fmt::Display;
use strum::Display;

use crate::{
    duconodetypes::{NodeType, VentilationPosition},
    modbus::DucoModbusConnection,
    mqtt::MqttData,
    MqttBridgeError,
};

#[derive(Clone, Copy, Display, Debug)]
#[strum(serialize_all = "snake_case")]
enum InputRegister {
    SystemType = 0,
    //RemainingTimeCurrentVenilationMode = 2,
    FlowRateVsTargetLevel = 3,
    //IndoorAirQualityBasedOnRH = 4,
    IndoorAirQualityBasedOnCO2 = 5,
    //VentilationStatus = 6,
    FilterTimeRemaining = 7,
}

#[derive(Clone, Copy, Display, Debug)]
#[strum(serialize_all = "snake_case")]
enum HoldingRegister {
    VentilationPosition = 0,
    Identification = 1,
    SupplyTemperatureTargetZone1 = 2,
    SupplyTemperatureTargetZone2 = 3,
}

const UNKNOWN: &str = "UNKNOWN";

fn optional_enum_string<T: Display + num::FromPrimitive>(val: u16) -> String {
    let enum_type: Option<T> = num::FromPrimitive::from_u16(val);
    match enum_type {
        Some(enum_type) => format!("{enum_type}"),
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
    registers: Vec<Register>,
}

impl DucoBoxNode {
    pub fn create_duco_box_node(number: u16) -> DucoBoxNode {
        DucoBoxNode {
            number,
            registers: vec![
                Register::Input(InputRegister::SystemType, RegisterValue::new()),
                Register::Input(InputRegister::FlowRateVsTargetLevel, RegisterValue::new()),
                Register::Input(InputRegister::FilterTimeRemaining, RegisterValue::new()),
                Register::Holding(HoldingRegister::VentilationPosition, RegisterValue::new()),
                Register::Holding(
                    HoldingRegister::SupplyTemperatureTargetZone1,
                    RegisterValue::new(),
                ),
                Register::Holding(
                    HoldingRegister::SupplyTemperatureTargetZone2,
                    RegisterValue::new(),
                ),
            ],
        }
    }

    pub fn create_co2_room_sensor_node(number: u16) -> DucoBoxNode {
        DucoBoxNode {
            number,
            registers: vec![
                Register::Input(InputRegister::SystemType, RegisterValue::new()),
                Register::Input(
                    InputRegister::IndoorAirQualityBasedOnCO2,
                    RegisterValue::new(),
                ),
                Register::Holding(HoldingRegister::VentilationPosition, RegisterValue::new()),
                Register::Holding(HoldingRegister::Identification, RegisterValue::new()),
            ],
        }
    }

    pub fn create_sensorless_control_valve(number: u16) -> DucoBoxNode {
        DucoBoxNode {
            number,
            registers: vec![
                Register::Input(InputRegister::SystemType, RegisterValue::new()),
                Register::Input(InputRegister::FlowRateVsTargetLevel, RegisterValue::new()),
                Register::Holding(HoldingRegister::VentilationPosition, RegisterValue::new()),
            ],
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

    pub async fn update_status(
        &mut self,
        modbus: &mut DucoModbusConnection,
    ) -> Result<(), MqttBridgeError> {
        for register in self.registers.iter_mut() {
            match register {
                Register::Input(reg, val) => {
                    let addr = self.number * 100 + *reg as u16;
                    let register_val = modbus.read_input_register(addr).await;
                    if let Err(err) = register_val.as_ref() {
                        log::warn!("Failed to read input register: {err}")
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
                | InputRegister::FilterTimeRemaining => {
                    format!("{val}")
                }
            },
            None => String::from(UNKNOWN),
        }
    }

    fn register_topic<T: Display>(node_number: u16, reg: T) -> String {
        format!("node_{}/{}", node_number, reg.to_string())
    }

    fn holding_register_value_string(reg: HoldingRegister, val: Option<u16>) -> String {
        match val {
            Some(val) => match reg {
                HoldingRegister::VentilationPosition => {
                    optional_enum_string::<VentilationPosition>(val)
                }
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
