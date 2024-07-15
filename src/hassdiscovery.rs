use crate::Result;
use serde::Serialize;

use crate::{duxoboxnode::DucoBoxNode, mqtt::MqttData};

const HASS_DISCOVERY_TOPIC: &str = "homeassistant";

#[derive(Serialize)]
pub struct Origin {
    name: String,
    sw: String,
    url: String,
}

impl Origin {
    pub fn duco2mqtt() -> Origin {
        Origin {
            name: String::from(env!("CARGO_PKG_NAME")),
            sw: String::from(env!("CARGO_PKG_VERSION")),
            url: String::from("https://github.com/dirkvdb/duco2mqtt"),
        }
    }
}

#[derive(Serialize)]
pub struct Sensor {
    pub origin: Origin,
    pub name: String,
    pub obj_id: String,
    pub unique_id: String,
    pub stat_t: String,
    pub avty_t: String,
    pub state_class: Option<String>,
    pub unit_of_measurement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Serialize)]
pub struct Number {
    pub origin: Origin,
    pub name: String,
    pub obj_id: String,
    pub unique_id: String,
    pub stat_t: String,
    pub avty_t: String,
    pub cmd_t: String,
    pub device_class: Option<String>,
    pub min: Option<i32>,
    pub max: Option<i32>,
    pub unit_of_measurement: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Serialize)]
pub struct Select {
    pub origin: Origin,
    pub name: String,
    pub obj_id: String,
    pub unique_id: String,
    pub stat_t: String,
    pub avty_t: String,
    pub cmd_t: String,
    pub options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Serialize)]
pub struct Light {
    pub origin: Origin,
    pub name: String,
    pub obj_id: String,
    pub unique_id: String,
    pub stat_t: String,
    pub avty_t: String,
    pub cmd_t: String,
    pub payload_on: String,
    pub payload_off: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

pub fn create_sensor_for_status(node_nr: u16, base_topic: &str, topic_name: &str, status: &str) -> Sensor {
    let unique_id = format!("duco_node_{}_{}", node_nr, status);

    Sensor {
        origin: Origin::duco2mqtt(),
        name: String::from(topic_name),
        obj_id: unique_id.clone(),
        unique_id,
        stat_t: format!("{}duco_node_{}/{}", base_topic, node_nr, topic_name),
        avty_t: format!("{}state", base_topic),
        state_class: None,
        unit_of_measurement: None,
        icon: None,
    }
}

pub fn create_light_for_status(
    node_nr: u16,
    base_topic: &str,
    topic_name: &str,
    cmd_topic_name: &str,
    status: &str,
) -> Light {
    let unique_id = format!("duco_node_{}_{}", node_nr, status);

    Light {
        origin: Origin::duco2mqtt(),
        name: String::from(topic_name),
        obj_id: unique_id.clone(),
        unique_id,
        stat_t: format!("{}duco_node_{}/{}", base_topic, node_nr, topic_name),
        avty_t: format!("{}state", base_topic),
        cmd_t: format!("{}duco_node_{}/cmnd/{}", base_topic, node_nr, cmd_topic_name),
        payload_on: String::from("1"),
        payload_off: String::from("0"),
        icon: None,
    }
}

// pub fn create_number_for_register<TReg: std::string::ToString>(
//     node_nr: u16,
//     base_topic: &str,
//     topic_name: &str,
//     reg: TReg,
// ) -> Number {
//     let unique_id = format!("duco_node_{}_{}", node_nr, reg.to_string());

//     Number {
//         origin: Origin::duco2mqtt(),
//         name: String::from(topic_name),
//         obj_id: unique_id.clone(),
//         unique_id,
//         stat_t: format!("{}node_{}/{}", base_topic, node_nr, topic_name),
//         avty_t: format!("{}state", base_topic),
//         cmd_t: format!("{}node_{}/cmnd/{}", base_topic, node_nr, topic_name),
//         device_class: None,
//         min: None,
//         max: None,
//         unit_of_measurement: None,
//         icon: None,
//     }
// }

pub fn create_select_for_status(
    node_nr: u16,
    base_topic: &str,
    topic_name: &str,
    cmd_topic_name: &str,
    status: &str,
    valid_states: &[String],
) -> Select {
    let unique_id = format!("duco_node_{}_{}", node_nr, status);

    Select {
        origin: Origin::duco2mqtt(),
        name: String::from(topic_name),
        obj_id: unique_id.clone(),
        unique_id,
        stat_t: format!("{}duco_node_{}/{}", base_topic, node_nr, topic_name),
        avty_t: format!("{}state", base_topic),
        cmd_t: format!("{}duco_node_{}/cmnd/{}", base_topic, node_nr, cmd_topic_name),
        options: Vec::from(valid_states),
        icon: None,
    }
}

pub fn ventilation_state_topic(node: &DucoBoxNode, base_topic: &str, valid_states: &[String]) -> Result<MqttData> {
    let mut select = create_select_for_status(
        node.number(),
        base_topic,
        "ventilation/State",
        "SetVentilationState",
        "ventilation_state",
        valid_states,
    );
    select.icon = Some(String::from("mdi:fan"));

    Ok(MqttData {
        topic: format!("{}/select/{}/config", HASS_DISCOVERY_TOPIC, select.unique_id),
        payload: serde_json::to_string(&select)?,
    })
}

pub fn flow_level_target_topic(node: &DucoBoxNode, base_topic: &str) -> Result<MqttData> {
    let mut sensor = create_sensor_for_status(
        node.number(),
        base_topic,
        "ventilation/FlowLvlTgt",
        "ventilation_flow_level_target",
    );
    sensor.state_class = Some(String::from("measurement"));
    sensor.unit_of_measurement = Some(String::from("%"));
    sensor.icon = Some(String::from("mdi:fan-clock"));

    Ok(MqttData {
        topic: format!("{}/sensor/{}/config", HASS_DISCOVERY_TOPIC, sensor.unique_id),
        payload: serde_json::to_string(&sensor)?,
    })
}

pub fn co2_sensor_topic(node: &DucoBoxNode, base_topic: &str) -> Result<MqttData> {
    let mut sensor = create_sensor_for_status(node.number(), base_topic, "sensor/IaqCo2", "sensor_iaq_co2");
    sensor.state_class = Some(String::from("measurement"));
    sensor.unit_of_measurement = Some(String::from("%"));
    sensor.icon = Some(String::from("mdi:molecule-co2"));

    Ok(MqttData {
        topic: format!("{}/sensor/{}/config", HASS_DISCOVERY_TOPIC, sensor.unique_id),
        payload: serde_json::to_string(&sensor)?,
    })
}

pub fn state_time_remaining_topic(node: &DucoBoxNode, base_topic: &str) -> Result<MqttData> {
    let mut sensor = create_sensor_for_status(
        node.number(),
        base_topic,
        "ventilation/TimeStateRemain",
        "ventilation_state_time_remaining",
    );
    sensor.state_class = Some(String::from("measurement"));
    sensor.unit_of_measurement = Some(String::from("seconds"));
    sensor.icon = Some(String::from("mdi:timer"));

    Ok(MqttData {
        topic: format!("{}/sensor/{}/config", HASS_DISCOVERY_TOPIC, sensor.unique_id),
        payload: serde_json::to_string(&sensor)?,
    })
}

pub fn identify_topic(node: &DucoBoxNode, base_topic: &str) -> Result<MqttData> {
    let mut light = create_light_for_status(node.number(), base_topic, "general/Identify", "SetIdentify", "identify");
    light.icon = Some(String::from("mdi:led-on"));

    Ok(MqttData {
        topic: format!("{}/light/{}/config", HASS_DISCOVERY_TOPIC, light.unique_id),
        payload: serde_json::to_string(&light)?,
    })
}
