use std::time::Duration;

use rumqttc::v5::{
    mqttbytes::{
        v5::{ConnectReturnCode, LastWill, Packet},
        QoS,
    },
    AsyncClient, Event, EventLoop, MqttOptions,
};

use crate::Error;

#[derive(Clone)]
pub struct MqttConfig {
    pub server: String,
    pub port: u16,
    pub client_id: String,
    pub user: String,
    pub password: String,
    pub base_topic: String,
}

#[derive(Debug, PartialEq, Eq)]
pub struct MqttData {
    pub topic: String,
    pub payload: String,
}

impl MqttData {
    pub fn new<T: AsRef<str>>(topic: T, payload: T) -> MqttData {
        MqttData {
            topic: String::from(topic.as_ref()),
            payload: String::from(payload.as_ref()),
        }
    }
}

impl PartialOrd for MqttData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.topic.cmp(&other.topic))
    }
}

impl Ord for MqttData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.topic.cmp(&other.topic)
    }
}

pub struct MqttConnection {
    client: AsyncClient,
    eventloop: EventLoop,
    base_topic: String,
    online: bool,
}

fn from_mqtt_string(stream: &bytes::Bytes) -> Result<String, Error> {
    match String::from_utf8(stream.to_vec()) {
        Ok(v) => Ok(v),
        Err(e) => Err(Error::Utf8Error(e.utf8_error())),
    }
}

const OFFLINE_PAYLOAD: &str = "offline";
const ONLINE_PAYLOAD: &str = "online";

fn state_topic(base_topic: &String) -> String {
    format!("{}/state", base_topic)
}

impl MqttConnection {
    pub fn new(cfg: MqttConfig) -> MqttConnection {
        let mut mqttoptions = MqttOptions::new(cfg.client_id, cfg.server, cfg.port);
        mqttoptions.set_clean_start(true);
        mqttoptions.set_keep_alive(Duration::from_secs(180));
        mqttoptions.set_last_will(LastWill::new(
            state_topic(&cfg.base_topic),
            OFFLINE_PAYLOAD,
            QoS::AtLeastOnce,
            true,
            None,
        ));

        if !cfg.user.is_empty() {
            mqttoptions.set_credentials(cfg.user, cfg.password);
        }

        let (client, eventloop) = AsyncClient::new(mqttoptions, 100);

        log::info!("MQTT connection created");
        MqttConnection {
            client,
            eventloop,
            base_topic: cfg.base_topic,
            online: false,
        }
    }

    pub async fn poll(&mut self) -> Result<Option<MqttData>, Error> {
        if let Ok(msg) = self.eventloop.poll().await {
            return self.handle_mqtt_message(msg).await;
        }

        Ok(None)
    }

    pub async fn publish(&mut self, data: MqttData) -> Result<(), Error> {
        Ok(self
            .client
            .publish(data.topic, QoS::AtLeastOnce, true, data.payload)
            .await?)
    }

    async fn subscribe_to_commands(&mut self) -> Result<(), Error> {
        let cmd_subscription_topic = format!("{}/+/cmnd/+", self.base_topic);
        self.client.subscribe(cmd_subscription_topic, QoS::ExactlyOnce).await?;
        Ok(())
    }

    pub async fn publish_online(&mut self) -> Result<(), Error> {
        if !self.online {
            self.client
                .publish(state_topic(&self.base_topic), QoS::AtLeastOnce, true, ONLINE_PAYLOAD)
                .await?;
            self.online = true;
        }

        Ok(())
    }

    pub async fn publish_offline(&mut self) -> Result<(), Error> {
        if self.online {
            self.client
                .publish(state_topic(&self.base_topic), QoS::AtLeastOnce, true, OFFLINE_PAYLOAD)
                .await?;

            self.online = false;
        }

        Ok(())
    }

    async fn handle_mqtt_message(&mut self, ev: Event) -> Result<Option<MqttData>, Error> {
        if let Event::Incoming(event) = ev {
            match event {
                Packet::ConnAck(data) => {
                    if data.code == ConnectReturnCode::Success {
                        if !data.session_present {
                            log::info!("Subscribe to mqtt commands");
                            self.subscribe_to_commands().await?;
                        } else {
                            log::debug!("Session still active, no need to resubsribe");
                        }
                    } else {
                        log::error!("MQTT connection refused: {:?}", data.code);
                    }
                }
                Packet::Publish(publ) => {
                    return Ok(Some(MqttData {
                        topic: from_mqtt_string(&publ.topic)?,
                        payload: from_mqtt_string(&publ.payload)?,
                    }));
                }
                _ => {}
            }
        }

        Ok(None)
    }
}
