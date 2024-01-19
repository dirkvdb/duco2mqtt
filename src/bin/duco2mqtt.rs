#![warn(clippy::unwrap_used)]
use core::time;

use clap::Parser;
use duco2mqtt::{
    bridge::{self, DucoMqttBridgeConfig},
    modbus::ModbusConfig,
    mqtt::MqttConfig,
};
use env_logger::Env;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PACKAGE: &str = env!("CARGO_PKG_NAME");

#[derive(Parser, Debug)]
#[clap(name = "duco2mqtt", about = "Interface between duco connectivity board and MQTT")]
struct Opt {
    // set the duco connectivity board address
    #[clap(long = "duco-addr", env = "D2M_DUCO_ADDRESS")]
    duco_addr: String,

    #[clap(long = "duco-modbus-slave-id", env = "D2M_MODBUS_SLAVE_ID", default_value_t = 1)]
    duco_slave_id: u8,

    #[clap(long = "duco-poll-interval", env = "D2M_POLL_INTERVAL", default_value_t = 60)]
    duco_poll_interval: u64,

    // set the mqtt addr
    #[clap(long = "mqtt-addr", env = "D2M_MQTT_ADDRESS")]
    mqtt_addr: String,

    #[clap(long = "mqtt-user", env = "D2M_MQTT_USER")]
    mqtt_user: Option<String>,

    #[clap(long = "mqtt-pass", env = "D2M_MQTT_PASS")]
    mqtt_password: Option<String>,

    #[clap(long = "mqtt-port", env = "D2M_MQTT_PORT", default_value_t = 1883)]
    mqtt_port: u16,

    #[clap(long = "mqtt-client-id", env = "D2M_CLIENT_ID", default_value_t = String::from("duco2mqtt"))]
    mqtt_client_id: String,

    #[clap(long = "mqtt-base-topic", env = "D2M_MQTT_BASE_TOPIC", default_value_t = String::from("ventilation"))]
    mqtt_base_topic: String,

    #[clap(long = "hass-discovery", env = "D2M_HASS_DISCOVERY", default_value_t = false)]
    hass_discovery: bool,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opt = Opt::parse();

    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp(None)
        .init();

    log::info!("{} version {}", PACKAGE, VERSION);

    let cfg = DucoMqttBridgeConfig {
        modbus_config: ModbusConfig {
            slave_address: opt.duco_addr,
            slave_id: opt.duco_slave_id,
            poll_interval: time::Duration::from_secs(opt.duco_poll_interval),
        },
        mqtt_config: MqttConfig {
            server: opt.mqtt_addr,
            port: opt.mqtt_port,
            client_id: opt.mqtt_client_id,
            user: opt.mqtt_user.unwrap_or(String::new()),
            password: opt.mqtt_password.unwrap_or(String::new()),
            base_topic: opt.mqtt_base_topic,
        },
        hass_discovery: opt.hass_discovery,
    };

    bridge::DucoMqttBridge::new(cfg)
        .run()
        .await
        .expect("Failed to run bridge");
}
