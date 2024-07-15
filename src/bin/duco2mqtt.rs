#![warn(clippy::unwrap_used)]
use core::time;
use std::path::PathBuf;

use clap::Parser;
use clap_verbosity_flag::WarnLevel;
use duco2mqtt::{
    bridge::{self, DucoMqttBridgeConfig},
    mqtt::MqttConfig,
};
use env_logger::Env;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const PACKAGE: &str = env!("CARGO_PKG_NAME");

#[derive(Parser, Debug)]
#[clap(name = "duco2mqtt", about = "Interface between duco connectivity board and MQTT")]
struct Opt {
    #[command(flatten)]
    verbose: clap_verbosity_flag::Verbosity<WarnLevel>,

    // set the duco connectivity board host name
    #[clap(long = "duco-host", env = "D2M_DUCO_HOST")]
    duco_host: String,

    // set the duco connectivity board ip address (optional, to avoid dns lookup)
    #[clap(long = "duco-ip", env = "D2M_DUCO_IP_ADDRESS")]
    duco_ip: Option<String>,

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

    #[clap(long = "certificate", env = "D2M_DUCO_CERTIFICATE")]
    certificate: Option<String>,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let opt = Opt::parse();

    env_logger::Builder::from_env(Env::default())
        .format_timestamp(None)
        .filter_level(opt.verbose.log_level_filter())
        .init();

    log::info!("{} version {}", PACKAGE, VERSION);

    let cfg = DucoMqttBridgeConfig {
        ducobox_host: opt.duco_host.clone(),
        ducobox_ip_address: opt.duco_ip.clone(),
        ducobox_certificate: opt.certificate.map(PathBuf::from),
        poll_interval: time::Duration::from_secs(opt.duco_poll_interval),
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
