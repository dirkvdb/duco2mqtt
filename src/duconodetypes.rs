use strum::{Display, EnumString, VariantNames};

#[derive(FromPrimitive, EnumString, Display, Clone, Copy)]
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

#[derive(FromPrimitive, EnumString, VariantNames, Debug, Display)]
#[repr(u16)]
pub enum VentilationPosition {
    #[strum(serialize = "Auto")]
    AUTO = 0,
    #[strum(serialize = "Manual 1")]
    Manual1 = 4,
    #[strum(serialize = "Manual 2")]
    Manual2 = 5,
    #[strum(serialize = "Manual 3")]
    Manual3 = 6,
    #[strum(serialize = "Not at home")]
    NotAtHome = 7,
    #[strum(serialize = "Permanent 1")]
    Permanent1 = 8,
    #[strum(serialize = "Permanent 2")]
    Permanent2 = 9,
    #[strum(serialize = "Permanent 3")]
    Permanent3 = 10,
    #[strum(serialize = "Unknown")]
    Unknown = 11,
}

#[derive(Clone, Copy, EnumString, Display, Debug, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum InputRegister {
    SystemType = 0,
    RemainingTimeCurrentVenilationMode = 2,
    FlowRateVsTargetLevel = 3,
    //IndoorAirQualityBasedOnRH = 4,
    IndoorAirQualityBasedOnCO2 = 5,
    //VentilationStatus = 6,
    FilterTimeRemaining = 7,
}

#[derive(Clone, Copy, EnumString, Display, Debug, PartialEq)]
#[strum(serialize_all = "snake_case")]
pub enum HoldingRegister {
    VentilationPosition = 0,
    Identification = 1,
    SupplyTemperatureTargetZone1 = 2,
    SupplyTemperatureTargetZone2 = 3,
}
