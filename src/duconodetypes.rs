use strum::{Display, EnumString};

#[derive(FromPrimitive, EnumString, Display, Clone, Copy)]
#[repr(u16)]
pub enum NodeType {
    Unknown = 0,
    RemoteControlRFBAT = 8,
    RemoteControlRFWired = 9,
    HumidityRoomSensor = 10,
    #[strum(serialize = "UCCO2")]
    CO2RoomSensor = 12,
    SensorlessControlValve = 13,
    HumidityControlValve = 14,
    #[strum(serialize = "VLV")]
    CO2ControlValve = 16,
    #[strum(serialize = "BOX")]
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

// "auto" = AutomaticMode;
// "man1" = ManualMode1;
// "man2" = ManualMode2;
// "man3" = ManualMode3;
// "empt" = EmptyHouse;
// "alrm" = AlarmMode;
// "cnt1" = PermanentManualMode1;
// "cnt2" = PermanentManualMode2;
// "cnt3" = PermanentManualMode3;
// "aut0" = AutomaticMode;
// "aut1" = Boost10min;
// "aut2" = Boost20min;
// "aut3" = Boost30min;
