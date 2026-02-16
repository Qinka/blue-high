// 该文件是 BlueHigh 项目的一部分。
// src/lora.rs - LoRa 模块
//
// 本文件根据 Apache 许可证第 2.0 版（以下简称“许可证”）授权使用；
// 除非遵守该许可证条款，否则您不得使用本文件。
// 您可通过以下网址获取许可证副本：
// http://www.apache.org/licenses/LICENSE-2.0
// 除非适用法律要求或书面同意，根据本许可协议分发的软件均按“原样”提供，
// 不附带任何形式的明示或暗示的保证或条件。
// 有关许可权限与限制的具体条款，请参阅本许可协议。
//
// Copyright (C) 2026 Johann Li <me@qinka.pro>, Wareless Group

use core::ops::{DerefMut};

use stm32f1xx_hal::gpio::{Input, Output, Pin};
use stm32f1xx_hal::spi::{Instance, Spi};
use sx1268_rs::{Status, control::Control};

#[derive(Debug)]
pub enum ControlError<SE> {
  SpiError(SE),
  // PinError(PE),
}

fn spi_error<SE>(error: SE) -> sx1268_rs::Error<ControlError<SE>> {
  sx1268_rs::Error::ControlError(ControlError::SpiError(error))
}

// fn pin_error<SE, PE>(error: PE) -> sx1268_rs::Error<ControlError<SE, PE>> {
//   sx1268_rs::Error::ControlError(ControlError::PinError(error))
// }

/// Wrapper type to implement Control trait for Spi
pub struct LoraControl<
  W,
  S,
  const NRST_P: char,
  const NRST_N: u8,
  NrstMode,
  const CS_P: char,
  const CS_N: u8,
  CsMode,
  const BUSY_P: char,
  const BUSY_N: u8,
  BusyMode,
  const TX_P: char,
  const TX_N: u8,
  TxMode,
  const RX_P: char,
  const RX_N: u8,
  RxMode,
> where
  S: Instance,
{
  pub spi: Spi<S, W>,
  pub nrst_pin: Pin<NRST_P, NRST_N, Output<NrstMode>>,
  pub cs_pin: Pin<CS_P, CS_N, Output<CsMode>>,
  pub busy_pin: Pin<BUSY_P, BUSY_N, Input<BusyMode>>,
  pub tx_pin: Pin<TX_P, TX_N, Output<TxMode>>,
  pub rx_pin: Pin<RX_P, RX_N, Output<RxMode>>,
}

impl<
  S,
  const NRST_P: char,
  const NRST_N: u8,
  NrstMode,
  const CS_P: char,
  const CS_N: u8,
  CsMode,
  const BUSY_P: char,
  const BUSY_N: u8,
  BusyMode,
  const TX_P: char,
  const TX_N: u8,
  TxMode,
  const RX_P: char,
  const RX_N: u8,
  RxMode,
> Control
  for LoraControl<
    u8,
    S,
    NRST_P,
    NRST_N,
    NrstMode,
    CS_P,
    CS_N,
    CsMode,
    BUSY_P,
    BUSY_N,
    BusyMode,
    TX_P,
    TX_N,
    TxMode,
    RX_P,
    RX_N,
    RxMode,
  >
where
  S: Instance,
{
  type Status = Status;
  type Error = sx1268_rs::Error<ControlError<stm32f1xx_hal::spi::Error>>;

  // -----------------------------------------------------------------------
  // Low-level SPI helpers
  // -----------------------------------------------------------------------

  /// Write a command with parameters.
  fn write_command(&mut self, opcode: u8, params: &[u8]) -> Result<(), Self::Error> {
    while self.busy_pin.is_high() {}
    self.cs_pin.set_low();
    self.spi.deref_mut().write(&[opcode]).map_err(spi_error)?;
    self.spi.deref_mut().write(params).map_err(spi_error)?;
    defmt::trace!("SPI write cmd=0x{:02X} params={:?}", opcode, params);
    self.cs_pin.set_high();
    Ok(())
  }

  /// Read a command response.
  /// The SX1268 protocol sends a status byte after the opcode + NOP, then
  /// returns the response data.
  fn read_command(&mut self, opcode: u8, response: &mut [u8]) -> Result<Status, Self::Error> {
    while self.busy_pin.is_high() {}
    self.cs_pin.set_low();
    self.spi.deref_mut().write(&[opcode]).map_err(spi_error)?;
    self.spi.deref_mut().read(response).map_err(spi_error)?;
    self.cs_pin.set_high();

    let status = Status::from(0);
    defmt::trace!(
      "SPI read cmd=0x{:02X} status={} resp={:?}",
      opcode,
      status,
      response
    );
    Ok(status)
  }

  /// Write to registers starting at the given address.
  fn write_register(&mut self, address: u16, data: &[u8]) -> Result<(), Self::Error> {
    let header = [0x0D, (address >> 8) as u8, address as u8];
    while self.busy_pin.is_high() {}
    self.cs_pin.set_low();
    self.spi.deref_mut().write(&header).map_err(spi_error)?;
    self.spi.deref_mut().write(data).map_err(spi_error)?;
    self.cs_pin.set_high();
    defmt::trace!("WriteRegister addr=0x{:04X} data={:?}", address, data);
    Ok(())
  }

  /// Read from registers starting at the given address.
  fn read_register(&mut self, address: u16, data: &mut [u8]) -> Result<(), Self::Error> {
    let header = [0x1D, (address >> 8) as u8, address as u8];
    while self.busy_pin.is_high() {}
    self.cs_pin.set_low();
    self.spi.deref_mut().write(&header).map_err(spi_error)?;
    self.spi.deref_mut().read(data).map_err(spi_error)?;
    self.cs_pin.set_high();
    defmt::info!("ReadRegister addr=0x{:04X} data={:?}", address, data);
    // Status not reliably returned from read_register, return default
    // Ok(Status::from(0))
    Ok(())
  }

  /// Write data to the TX buffer at the given offset.
  fn write_buffer(&mut self, offset: u8, data: &[u8]) -> Result<(), Self::Error> {
    let header = [sx1268_rs::codes::WRITE_BUFFER, offset];
    while self.busy_pin.is_high() {}
    self.cs_pin.set_low();
    self.spi.deref_mut().write(&header).map_err(spi_error)?;
    self.spi.deref_mut().write(data).map_err(spi_error)?;
    self.cs_pin.set_high();
    defmt::trace!("WriteBuffer offset={} len={}", offset, data.len());
    Ok(())
  }

  /// Read data from the RX buffer at the given offset.
  fn read_buffer(&mut self, offset: u8, data: &mut [u8]) -> Result<(), Self::Error> {
    let header = [sx1268_rs::codes::READ_BUFFER, offset];
    while self.busy_pin.is_high() {}
    self.cs_pin.set_low();
    self.spi.deref_mut().write(&header).map_err(spi_error)?;
    self.spi.deref_mut().read(data).map_err(spi_error)?;
    self.cs_pin.set_high();
    defmt::trace!("ReadBuffer offset={} len={}", offset, data.len());
    Ok(())
  }

  /// Get the device status.
  fn get_status(&mut self) -> Result<Status, Self::Error> {
    let mut status_byte = [0u8; 1];
    while self.busy_pin.is_high() {}
    self.cs_pin.set_low();
    self.spi.deref_mut().write(&[0xC0]).map_err(spi_error)?;
    self
      .spi
      .deref_mut()
      .read(&mut status_byte)
      .map_err(spi_error)?;
    self.cs_pin.set_high();
    let status = Status::from(status_byte[0]);
    defmt::debug!("GetStatus status={}", status);
    Ok(status)
  }

  fn reset(&mut self) -> Result<(), Self::Error> {
    self.nrst_pin.set_low();
    cortex_m::asm::delay(10_000); // 10ms delay
    self.nrst_pin.set_high();
    cortex_m::asm::delay(10_000); // 10ms delay
    Ok(())
  }

  fn wakeup(&mut self) -> Result<(), Self::Error> {
    // To wake up from sleep, just toggle CS
    self.cs_pin.set_low();
    cortex_m::asm::delay(10); // Short delay
    self.cs_pin.set_high();
    cortex_m::asm::delay(10); // Short delay
    Ok(())
  }

  fn switch_rx(&mut self, _: u32) -> Result<(), Self::Error> {
    self.tx_pin.set_low();
    self.rx_pin.set_high();
    Ok(())
  }

  fn switch_tx(&mut self, _: u32) -> Result<(), Self::Error> {
    self.rx_pin.set_low();
    self.tx_pin.set_high();
    Ok(())
  }
}
