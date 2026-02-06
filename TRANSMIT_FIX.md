# LoRa 发送问题修复总结

## 问题描述

通过串口（USB CDC）传递过来的数据无法被 LoRa 发射。

## 根本原因

在 `src/sx1268_driver.rs` 的 `update_packet_length()` 函数中，使用了错误的实现：

```rust
// 错误的实现
fn update_packet_length(&mut self, payload_len: u8) -> Result<(), ()> {
    let data = [0x00, 0x00, 0x00, payload_len, 0x00, 0x00]; // ❌ 全零 dummy 值
    let cmd = [commands::SET_PACKET_PARAMS];
    self.hal.write(&cmd, &data)
}
```

### 问题影响

当调用 `SET_PACKET_PARAMS` 命令时，SX1268 芯片需要 6 个字节的参数：
1. Byte 0-1: 前导码长度（应该是 8，被设为 0）
2. Byte 2: 头部类型（应该是 0x00 显式，被设为 0x00 但实际是巧合）
3. Byte 3: 有效负载长度（正确设置）
4. Byte 4: CRC 启用（应该是 0x01，被设为 0x00 **禁用**）
5. Byte 5: IQ 极性反转（应该是 0x00，被设为 0x00 巧合正确）

关键问题：
- **前导码长度被设为 0**：接收器无法检测到信号
- **CRC 被禁用**：数据完整性检查失效

## 解决方案

### 1. 添加状态保存

在 `Sx1268Driver` 结构体中添加字段：

```rust
pub struct Sx1268Driver<...> {
    hal: Sx1268Context<...>,
    preamble_length: u16,     // 保存前导码长度
    header_type: u8,          // 保存头部类型
    crc_enabled: bool,        // 保存 CRC 启用状态
    invert_iq: bool,          // 保存 IQ 反转状态
}
```

### 2. 初始化时保存参数

在 `set_lora_packet_params()` 中：

```rust
fn set_lora_packet_params(&mut self, config: &LoRaConfig) -> Result<(), ()> {
    // ... 计算参数 ...
    
    // 保存参数以便后续发送时使用
    self.preamble_length = config.preamble_length;
    self.header_type = header_type;
    self.crc_enabled = config.crc_enabled;
    self.invert_iq = false;
    
    // ... 写入芯片 ...
}
```

### 3. 更新长度时使用保存的参数

修复后的 `update_packet_length()`:

```rust
fn update_packet_length(&mut self, payload_len: u8) -> Result<(), ()> {
    // 使用保存的参数
    let preamble = self.preamble_length.to_be_bytes();
    let crc = if self.crc_enabled { 0x01 } else { 0x00 };
    let invert_iq = if self.invert_iq { 0x01 } else { 0x00 };
    
    let data = [
        preamble[0], preamble[1],  // ✅ 保留前导码长度
        self.header_type,           // ✅ 保留头部类型
        payload_len,                // ✅ 更新负载长度
        crc,                        // ✅ 保留 CRC 设置
        invert_iq                   // ✅ 保留 IQ 设置
    ];
    let cmd = [commands::SET_PACKET_PARAMS];
    self.hal.write(&cmd, &data)
}
```

## 测试验证

### 正确的发送流程

```
1. USB CDC 接收数据 (如 "Hello LoRa!")
2. [SX1268] 发送 12 字节
3. [SX1268] 进入待机模式
4. [SX1268] 写入缓冲区
5. [SX1268] 数据包长度更新: 12 字节 (保留其他参数) ✅
6. [SX1268] 配置发送中断
7. [SX1268] RF 开关 -> TX
8. [SX1268] 执行发送命令
9. [SX1268] 发送完成
10. [主循环] ✅ LoRa 发送成功
```

### 接收器验证

使用另一个配置相同的 LoRa 设备：
- 频率：433 MHz
- 带宽：500 kHz
- 扩频因子：SF11
- 编码率：CR4/5
- 前导码：8
- CRC：启用
- 同步字：0x14（私网）

应该能够成功接收到数据。

## 技术细节

### SX1268 SET_PACKET_PARAMS 命令

| 字节 | 参数 | 说明 |
|------|------|------|
| 0-1 | PreambleLength[15:0] | 前导码长度（符号数） |
| 2 | HeaderType | 0x00=显式, 0x01=隐式 |
| 3 | PayloadLength | 有效负载长度（字节） |
| 4 | CRC | 0x00=禁用, 0x01=启用 |
| 5 | InvertIQ | 0x00=标准, 0x01=反转 |

### 为什么前导码和 CRC 很重要

**前导码**:
- 接收器通过检测前导码来同步
- 前导码长度为 0 会导致接收器无法检测到信号
- 典型值：8-12 符号

**CRC**:
- 用于数据完整性检查
- 禁用 CRC 会导致接收器无法验证数据
- LoRa 通信中强烈建议启用

## 提交记录

- 提交：`0da6f1e`
- 标题：Fix SX1268 transmit by preserving packet parameters when updating payload length
- 日期：2026-02-06

## 总结

通过保存并正确使用数据包参数，修复了 LoRa 发送问题。现在 USB CDC 接收到的数据可以正确通过 E22-400M30S LoRa 模块发送出去。
