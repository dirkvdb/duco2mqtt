use std::time::Duration;

use rumqttc::v5::{
    mqttbytes::{
        v5::{ConnectReturnCode, Packet},
        QoS,
    },
    AsyncClient, Event, EventLoop, MqttOptions,
};

use crate::MqttBridgeError;

#[derive(Clone)]
pub struct MqttConfig {
    pub server: String,
    pub port: u16,
    pub client_id: String,
}

pub struct MqttData {
    pub topic: String,
    pub payload: String,
}

pub struct MqttConnection {
    client: AsyncClient,
    eventloop: EventLoop,
}

fn from_mqtt_string(stream: &bytes::Bytes) -> Result<String, MqttBridgeError> {
    match String::from_utf8(stream.to_vec()) {
        Ok(v) => Ok(v),
        Err(_e) => Err(MqttBridgeError::ParseError),
    }
}

impl MqttConnection {
    pub fn new(cfg: MqttConfig) -> MqttConnection {
        let mut mqttoptions = MqttOptions::new(cfg.client_id, cfg.server, cfg.port);
        mqttoptions.set_clean_start(true);
        mqttoptions.set_keep_alive(Duration::from_secs(180));

        let (client, eventloop) = AsyncClient::new(mqttoptions, 10);

        log::info!("MQTT connection created");
        MqttConnection { client, eventloop }
    }

    pub async fn poll(&mut self) -> Result<Option<MqttData>, MqttBridgeError> {
        if let Ok(msg) = self.eventloop.poll().await {
            return self.handle_mqtt_message(msg);
        }

        Ok(None)
    }

    async fn publish(&mut self, data: MqttData) -> Result<(), MqttBridgeError> {
        Ok(self
            .client
            .publish(data.topic, QoS::AtLeastOnce, true, data.payload)
            .await?)
    }

    fn handle_mqtt_message(&mut self, ev: Event) -> Result<Option<MqttData>, MqttBridgeError> {
        match ev {
            Event::Incoming(event) => match event {
                Packet::ConnAck(data) => {
                    if !data.session_present {
                        if data.code == ConnectReturnCode::Success {
                            log::info!("Subscribe to mqtt command data");
                            //self.subscribe_to_data().await?;
                        } else {
                            log::error!("MQTT connection refused: {:?}", data.code);
                        }
                    } else {
                        log::debug!("Session still active, no need to resubsribe");
                    }
                }
                Packet::Publish(publ) => {
                    return Ok(Some(MqttData {
                        topic: from_mqtt_string(&publ.topic)?,
                        payload: from_mqtt_string(&publ.payload)?,
                    }));
                }
                //Packet::Connect(_) => todo!(),
                // Packet::PubAck(_) => todo!(),
                // Packet::PubRec(_) => todo!(),
                // Packet::PubRel(_) => todo!(),
                // Packet::PubComp(_) => todo!(),
                // Packet::Subscribe(_) => todo!(),
                // Packet::SubAck(_) => todo!(),
                // Packet::Unsubscribe(_) => todo!(),
                // Packet::UnsubAck(_) => todo!(),
                // Packet::PingReq => todo!(),
                // Packet::PingResp => todo!(),
                // Packet::Disconnect => todo!(),
                _ => {}
            },
            _ => {}
        }

        Ok(None)
    }
}
