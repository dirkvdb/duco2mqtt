#![warn(clippy::unwrap_used)]
use clap::Parser;
use duco2mqtt::{
    bridge::{self, DucoMqttBridgeConfig},
    modbus::ModbusConfig,
    mqtt::MqttConfig,
};
use env_logger::{Env, TimestampPrecision};

#[derive(Parser, Debug)]
#[clap(name = "duco2mqtt", about = "Interface between duco connectivity board and MQTT")]
struct Opt {
    // set the duco connectivity board address
    #[clap(long = "duco-addr", env = "D2M_DUCO_ADDRESS")]
    duco_addr: String,

    #[clap(long = "duco-slave-id", env = "D2M_MODBUS_SLAVE_ID", default_value_t = 1)]
    duco_slave_id: u8,

    // set the mqtt addr
    #[clap(long = "mqtt-addr", env = "D2M_MQTT_ADDRESS")]
    mqtt_addr: String,

    #[clap(long = "mqtt-port", env = "D2M_MQTT_PORT", default_value_t = 1883)]
    mqtt_port: u16,

    #[clap(long = "mqtt-client-id", env = "D2M_CLIENT_ID", default_value_t = String::from("duco2mqtt"))]
    mqtt_client_id: String,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opt = Opt::parse();

    env_logger::Builder::from_env(Env::default().default_filter_or("info"))
        .format_timestamp(Some(TimestampPrecision::Millis))
        .init();

    let cfg = DucoMqttBridgeConfig {
        modbus_config: ModbusConfig {
            slave_address: opt.duco_addr,
            slave_id: opt.duco_slave_id,
        },
        mqtt_config: MqttConfig {
            server: opt.mqtt_addr,
            port: opt.mqtt_port,
            client_id: opt.mqtt_client_id,
        },
    };

    bridge::DucoMqttBridge::new(cfg)
        .run()
        .await
        .expect("Failed to run bridge");
}
