use strum::{Display, EnumString};

use crate::{modbus::DucoModbusConnection, mqtt::MqttData, MqttBridgeError};

pub trait DucoNode {
    fn number(&self) -> u16;
    fn topics_that_need_updating(&self) -> Vec<MqttData>;
    async fn update_status(&mut self, modbus: &mut DucoModbusConnection) -> Result<(), MqttBridgeError>;
}

#[derive(FromPrimitive, EnumString, Display)]
pub enum NodeType {
    Unknown = 0,
    RemoteControlRFBAT = 8,
    RemoteControlRFWired = 9,
    HumidityRoomSensor = 10,
    CO2RoomSensor = 12,
    SensorlessControlValve = 13,
    HumidityControlValve = 14,
    CO2ControlValve = 16,
    DucoBox = 17,
    SwitchSensor = 18,
    ControlUnit = 27,
    CO2RHControlValve = 28,
    RemoteControlSunControlRFWired = 29,
    RemoteControlNightventRFWired = 30,
    ExternalMultiZoneValve = 31,
    HumidityBoxSensor = 35,
    CO2BoxSensors = 37,
    DucoWeatherStation = 39,
}

#[derive(FromPrimitive, EnumString, Display, Debug)]
pub enum VentilationPosition {
    AUTO = 0,
    Manual1 = 4,
    Manual2 = 5,
    Manual3 = 6,
    NotAtHome = 7,
    Permanent1 = 8,
    Permanent2 = 9,
    Permanent3 = 10,
    Unknown = 11,
}
