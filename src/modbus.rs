use std::time::Duration;

use tokio_modbus::{
    client::{tcp, Context, Reader, Writer},
    Slave,
};

use crate::MqttBridgeError;

const CONNECTION_TIME: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct ModbusConfig {
    pub slave_address: String,
    pub slave_id: u8,
    pub poll_interval: std::time::Duration,
}

pub struct DucoModbusConnection {
    modbus: Option<Context>,
}

impl DucoModbusConnection {
    pub fn new() -> DucoModbusConnection {
        DucoModbusConnection { modbus: None }
    }

    pub async fn check_connection(&mut self) -> bool {
        match self.modbus.as_mut() {
            Some(modbus) => {
                if let Err(_) = modbus.read_input_registers(0, 1).await {
                    self.modbus = None;
                    false
                } else {
                    true
                }
            }
            None => false,
        }
    }

    pub async fn connect(&mut self, cfg: &ModbusConfig) -> Result<(), MqttBridgeError> {
        let socket_addr = format!("{}:502", cfg.slave_address).parse().unwrap();
        match tokio::time::timeout(
            CONNECTION_TIME,
            tcp::connect_slave(socket_addr, Slave(cfg.slave_id)),
        )
        .await
        {
            Ok(ok) => match ok {
                Ok(context) => self.modbus = Some(context),
                Err(err) => {
                    self.modbus = None;
                    return Err(MqttBridgeError::NetworkError(format!(
                        "Failed to connect to duco modbus server: {err}"
                    )));
                }
            },
            Err(err) => {
                self.modbus = None;
                return Err(MqttBridgeError::NetworkError(format!(
                    "Timeout connecting to duco modbus server ({err})"
                )));
            }
        }

        Ok(())
    }

    pub async fn disconnect(&mut self) -> Result<(), MqttBridgeError> {
        if let Some(conn) = self.modbus.as_mut() {
            conn.disconnect().await?;
        }

        Ok(())
    }

    pub async fn read_input_register(&mut self, address: u16) -> Result<u16, MqttBridgeError> {
        let result = self.connection()?.read_input_registers(address, 1).await?;
        Ok(result[0])
    }

    pub async fn read_input_registers(
        &mut self,
        address: u16,
        count: u16,
    ) -> Result<Vec<u16>, MqttBridgeError> {
        let result = self
            .connection()?
            .read_input_registers(address, count)
            .await?;
        Ok(result)
    }

    pub async fn read_holding_register(&mut self, address: u16) -> Result<u16, MqttBridgeError> {
        let result = self
            .connection()?
            .read_holding_registers(address, 1)
            .await?;
        Ok(result[0])
    }

    pub async fn write_holding_register(
        &mut self,
        address: u16,
        value: u16,
    ) -> Result<(), MqttBridgeError> {
        self.connection()?
            .write_single_register(address, value)
            .await?;
        Ok(())
    }

    fn connection(&mut self) -> Result<&mut Context, MqttBridgeError> {
        match self.modbus.as_mut() {
            Some(ctx) => Ok(ctx),
            None => Err(MqttBridgeError::RuntimeError(String::from(
                "Modbus not connected",
            ))),
        }
    }
}
