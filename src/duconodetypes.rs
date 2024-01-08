use strum::{Display, EnumString};

#[derive(FromPrimitive, EnumString, Display)]
#[repr(u16)]
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
#[repr(u16)]
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
