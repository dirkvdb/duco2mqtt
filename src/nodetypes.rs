use std::intrinsics::mir::Retag;

use crate::{
    duconode::{DucoNode, VentilationPosition},
    modbus::DucoModbusConnection,
    mqtt::MqttData,
    MqttBridgeError,
};
enum InputRegister {
    //SystemType = 100,
    //RemainingTimeCurrentVenilationMode = 102,
    FlowRateVsTargetLevel = 3,
    //IndoorAirQualityBasedOnRH = 104,
    IndoorAirQualityBasedOnCO2 = 5,
    //VentilationStatus = 106,
    FilterTimeRemaining = 7,
}

enum HoldingRegister {
    VentialtionPosition = 0,
    //Identification = 1,
    SupplyTemperatureTargetZone1 = 2,
    SupplyTemperatureTargetZone2 = 3,
}

enum RegisterType {
    Holding,
    Input,
}

struct RegisterValue<T> {
    reg_type: RegisterType,
    address: u16,
    value: Option<T>,
    modified: bool,
    name: String,
}

impl<T> RegisterValue<T> {
    fn new(reg_type: RegisterType, name: &str, address: u16) -> RegisterValue<T> {
        RegisterValue {
            reg_type,
            address,
            value: None,
            modified: true,
            name: String::from(name),
        }
    }

    fn name(&self) -> String {
        self.name
    }

    fn value_string(&self) -> String {
        match self.value {
            Some(val) => {format!("{val}")}
            None => String::from("UNKNOWN")
        }
    }

    fn set(&mut self, val: Option<T>) {
        if val != self.value {
            self.modified = true;
            self.value = val;
        }
    }

    fn modified(&self) -> bool {
        self.modified
    }

    fn get_and_reset(&mut self) -> Option<T> {
        self.modified = false;
        self.value
    }
}

struct NNode {
    Vec<
}

#[derive(Debug)]
pub struct DucoBoxNode {
    number: u16,
    flow_level_target: Option<u16>,
    filter_time_remaining: Option<u16>,
    supply_temperature_target_zone1: Option<u16>,
    supply_temperature_target_zone2: Option<u16>,
}

impl DucoBoxNode {
    pub fn new(number: u16) -> DucoBoxNode {
        DucoBoxNode {
            number,
            flow_level_target: None,
            filter_time_remaining: None,
            supply_temperature_target_zone1: None,
            supply_temperature_target_zone2: None,
        }
    }
}

impl DucoNode for DucoBoxNode {
    fn number(&self) -> u16 {
        return self.number;
    }

    fn topics_that_need_updating(&self) -> Vec<MqttData> {}

    async fn update_status(&mut self, modbus: &mut DucoModbusConnection) -> Result<(), MqttBridgeError> {
        self.set_flow_level_target(
            modbus
                .read_input_register(self.number * 100 + InputRegister::FlowRateVsTargetLevel as u16)
                .await
                .ok(),
        );

        self.set_filter_time_remaining(
            modbus
                .read_input_register(self.number * 100 + InputRegister::FilterTimeRemaining as u16)
                .await
                .ok(),
        );

        self.set_supply_temperature_target_zone1(
            modbus
                .read_holding_register(self.number * 100 + HoldingRegister::SupplyTemperatureTargetZone1 as u16)
                .await
                .ok(),
        );

        self.set_supply_temperature_target_zone2(
            modbus
                .read_holding_register(self.number * 100 + HoldingRegister::SupplyTemperatureTargetZone2 as u16)
                .await
                .ok(),
        );

        Ok(())
    }
}

#[derive(Debug)]
pub struct CO2RoomSensorNode {
    number: u16,
    air_quality: Option<u16>,
    mode: VentilationPosition,
}

impl CO2RoomSensorNode {
    pub fn new(number: u16) -> CO2RoomSensorNode {
        CO2RoomSensorNode {
            number,
            air_quality: None,
            mode: VentilationPosition::Unknown,
        }
    }
}

impl DucoNode for CO2RoomSensorNode {
    fn number(&self) -> u16 {
        return self.number;
    }

    fn topics_that_need_updating(&self) -> Vec<MqttData> {
        Vec::new()
    }

    async fn update_status(&mut self, modbus: &mut DucoModbusConnection) -> Result<(), MqttBridgeError> {
        self.air_quality = modbus
            .read_input_register(self.number * 100 + InputRegister::IndoorAirQualityBasedOnCO2 as u16)
            .await
            .ok();
        self.mode = num::FromPrimitive::from_u16(
            modbus
                .read_holding_register(self.number * 100 + HoldingRegister::VentialtionPosition as u16)
                .await
                .unwrap_or(11),
        )
        .unwrap_or(VentilationPosition::Unknown);
        Ok(())
    }
}

#[derive(Debug)]
pub struct SensorlessControlValveNode {
    number: u16,
    flow_level_target: Option<u16>,
    mode: VentilationPosition,
}

impl SensorlessControlValveNode {
    pub fn new(number: u16) -> SensorlessControlValveNode {
        SensorlessControlValveNode {
            number,
            flow_level_target: None,
            mode: VentilationPosition::Unknown,
        }
    }
}

impl DucoNode for SensorlessControlValveNode {
    fn number(&self) -> u16 {
        return self.number;
    }

    fn topics_that_need_updating(&self) -> Vec<MqttData> {
        Vec::new()
    }

    async fn update_status(&mut self, modbus: &mut DucoModbusConnection) -> Result<(), MqttBridgeError> {
        self.flow_level_target = modbus
            .read_input_register(self.number * 100 + InputRegister::FlowRateVsTargetLevel as u16)
            .await
            .ok();
        self.mode = num::FromPrimitive::from_u16(
            modbus
                .read_holding_register(self.number * 100 + HoldingRegister::VentialtionPosition as u16)
                .await
                .unwrap_or(11),
        )
        .unwrap_or(VentilationPosition::Unknown);
        Ok(())
    }
}
