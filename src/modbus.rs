use tokio_modbus::{
    client::{tcp, Context, Reader},
    Slave,
};

use crate::MqttBridgeError;

#[derive(Clone)]
pub struct ModbusConfig {
    pub slave_address: String,
    pub slave_id: u8,
}

pub struct DucoModbusConnection {
    cfg: ModbusConfig,
    modbus: Option<Context>,
}

impl DucoModbusConnection {
    pub fn new(cfg: ModbusConfig) -> DucoModbusConnection {
        DucoModbusConnection { cfg, modbus: None }
    }

    pub fn is_connected(&self) -> bool {
        self.modbus.is_some()
    }

    pub async fn connect(&mut self) -> Result<(), MqttBridgeError> {
        let socket_addr = format!("{}:502", self.cfg.slave_address).parse().unwrap();
        self.modbus = Some(tcp::connect_slave(socket_addr, Slave(self.cfg.slave_id)).await?);

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

    pub async fn read_input_registers(&mut self, address: u16, count: u16) -> Result<Vec<u16>, MqttBridgeError> {
        let result = self.connection()?.read_input_registers(address, count).await?;
        Ok(result)
    }

    pub async fn read_holding_register(&mut self, address: u16) -> Result<u16, MqttBridgeError> {
        let result = self.connection()?.read_holding_registers(address, 1).await?;
        Ok(result[0])
    }

    fn connection(&mut self) -> Result<&mut Context, MqttBridgeError> {
        match self.modbus.as_mut() {
            Some(ctx) => Ok(ctx),
            None => Err(MqttBridgeError::RuntimeError(String::from("Modbus not connected"))),
        }
    }
}
