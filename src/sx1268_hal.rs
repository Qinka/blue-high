//! SX1268 HAL 层实现
//! 
//! 基于 E22-400M30S 模块的硬件抽象层
//! 参考文档: https://www.ebyte.com/Uploadfiles/Files/2024-12-31/202412311627396369.pdf

use embedded_hal::spi::SpiBus;
use embedded_hal::digital::OutputPin;
use embedded_hal::digital::InputPin;
use defmt;

/// SX1268 HAL 状态码
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sx1268HalStatus {
    Ok,
    Error,
}

/// SX1268 HAL 上下文
pub struct Sx1268Context<SPI, NSS, NRST, BUSY, TXEN, RXEN> 
where
    SPI: SpiBus,
    NSS: OutputPin,
    NRST: OutputPin,
    BUSY: InputPin,
    TXEN: OutputPin,
    RXEN: OutputPin,
{
    pub spi: SPI,
    pub nss: NSS,
    pub nrst: NRST,
    pub busy: BUSY,
    pub txen: TXEN,
    pub rxen: RXEN,
}

impl<SPI, NSS, NRST, BUSY, TXEN, RXEN> Sx1268Context<SPI, NSS, NRST, BUSY, TXEN, RXEN>
where
    SPI: SpiBus,
    NSS: OutputPin,
    NRST: OutputPin,
    BUSY: InputPin,
    TXEN: OutputPin,
    RXEN: OutputPin,
{
    /// 创建新的 SX1268 HAL 上下文
    pub fn new(spi: SPI, nss: NSS, nrst: NRST, busy: BUSY, txen: TXEN, rxen: RXEN) -> Self {
        Self {
            spi,
            nss,
            nrst,
            busy,
            txen,
            rxen,
        }
    }

    /// 模块复位
    pub fn reset(&mut self, delay_fn: &mut dyn FnMut(u32)) -> Sx1268HalStatus {
        defmt::debug!("[SX1268] 执行硬件复位");
        
        // E22 RESET 引脚先拉低触发复位
        let _ = self.nrst.set_low();
        delay_fn(10);
        
        // E22 RESET 引脚再拉高恢复正常
        let _ = self.nrst.set_high();
        delay_fn(10);
        
        Sx1268HalStatus::Ok
    }

    /// 忙状态等待
    /// E22 BUSY 引脚高电平表示忙，需要等待
    pub fn wait_on_busy(&mut self) {
        const MAX_BUSY_WAIT_ITERATIONS: u32 = 10000;
        let mut timeout = MAX_BUSY_WAIT_ITERATIONS;
        while self.busy.is_high().unwrap_or(false) && timeout > 0 {
            timeout -= 1;
            // Small delay to avoid tight CPU loop
            cortex_m::asm::delay(100); // ~100 cycles delay
        }
        
        if timeout == 0 {
            defmt::warn!("[SX1268] BUSY 超时");
        }
    }

    /// 模块唤醒
    pub fn wakeup(&mut self, delay_fn: &mut dyn FnMut(u32)) -> Sx1268HalStatus {
        defmt::debug!("[SX1268] 唤醒模块");
        
        // E22 SPI CS(NSS) 引脚先拉低触发唤醒
        let _ = self.nss.set_low();
        delay_fn(1);
        
        // E22 SPI CS(NSS) 引脚再拉高恢复正常
        let _ = self.nss.set_high();
        delay_fn(1);
        
        Sx1268HalStatus::Ok
    }

    /// 寄存器写入
    pub fn write(&mut self, command: &[u8], data: &[u8]) -> Sx1268HalStatus {
        // 等待空闲
        self.wait_on_busy();
        
        // NSS 拉低选中
        let _ = self.nss.set_low();
        
        // SPI 发送命令
        for byte in command {
            let mut buf = [*byte];
            if let Err(_) = self.spi.transfer_in_place(&mut buf) {
                let _ = self.nss.set_high();
                return Sx1268HalStatus::Error;
            }
        }
        
        // SPI 发送数据
        for byte in data {
            let mut buf = [*byte];
            if let Err(_) = self.spi.transfer_in_place(&mut buf) {
                let _ = self.nss.set_high();
                return Sx1268HalStatus::Error;
            }
        }
        
        // NSS 拉高结束
        let _ = self.nss.set_high();
        
        Sx1268HalStatus::Ok
    }

    /// 寄存器读取
    /// 
    /// 注意：SX1268 的 SPI 协议是全双工的
    /// 在发送命令字节的同时，MISO 线上会返回响应数据
    /// 对于 READ_REGISTER，实际数据在发送最后一个字节（NOP）时返回
    pub fn read(&mut self, command: &[u8], data: &mut [u8]) -> Sx1268HalStatus {
        // 等待空闲
        self.wait_on_busy();
        
        // NSS 拉低选中
        let _ = self.nss.set_low();
        
        // SPI 发送命令，同时接收响应
        // 对于 READ_REGISTER [0x1D, Addr_H, Addr_L, NOP]：
        // - 前3个字节返回状态
        // - 第4个字节（NOP）返回实际寄存器数据
        let cmd_len = command.len();
        for (i, byte) in command.iter().enumerate() {
            let mut buf = [*byte];
            if let Err(_) = self.spi.transfer_in_place(&mut buf) {
                let _ = self.nss.set_high();
                return Sx1268HalStatus::Error;
            }
            
            // 如果这是最后一个命令字节（通常是 NOP），保存响应到 data
            if i == cmd_len - 1 && !data.is_empty() {
                data[0] = buf[0];
            }
        }
        
        // 如果 data 长度 > 1，继续读取额外的字节
        for byte in data.iter_mut().skip(1) {
            let mut buf = [0x00];
            if let Err(_) = self.spi.transfer_in_place(&mut buf) {
                let _ = self.nss.set_high();
                return Sx1268HalStatus::Error;
            }
            *byte = buf[0];
        }
        
        // NSS 拉高结束
        let _ = self.nss.set_high();
        
        Sx1268HalStatus::Ok
    }

    /// 射频开关切换到发送线路
    pub fn rf_switch_tx(&mut self) -> Sx1268HalStatus {
        defmt::debug!("[SX1268] RF 开关 -> TX");
        let _ = self.rxen.set_low();
        let _ = self.txen.set_high();
        Sx1268HalStatus::Ok
    }

    /// 射频开关切换到接收线路
    pub fn rf_switch_rx(&mut self) -> Sx1268HalStatus {
        defmt::debug!("[SX1268] RF 开关 -> RX");
        let _ = self.txen.set_low();
        let _ = self.rxen.set_high();
        Sx1268HalStatus::Ok
    }
    
    /// 关闭 RF 开关（待机）
    pub fn rf_switch_off(&mut self) -> Sx1268HalStatus {
        defmt::debug!("[SX1268] RF 开关 -> OFF");
        let _ = self.txen.set_low();
        let _ = self.rxen.set_low();
        Sx1268HalStatus::Ok
    }
}
