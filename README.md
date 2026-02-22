# proto_c

A procedural macro for generating Rust code from `.cproto` CAN frame definition files.

## Overview

This crate provides a `include_cproto!` macro that parses `.cproto` files and generates Rust code for working with CAN frames, similar to how Protocol Buffers work.

## `.cproto` File Format

The `.cproto` format supports:

### Enums
```
enum_t NodeId = {
    FCU = 0x00
    MCU = 0x01
}
```

### Byte Definitions (Bit Fields)
```
Byte Status = {
    StatusId = 0
    ErrorDetected = 1
    Reserved = ..
}
```

### Frame Definitions
```
frame MockFrame {
    ID {
        NodeId | Priority | 0x00
    }
    byte Status
}
```

## Usage

### 1. Add Dependency

Add to your `Cargo.toml`:

```toml
[dependencies]
proto_c = { path = "../proto_c" }
```

### 2. Create `.cproto` File

Create `protos/can.cproto`:

```
enum_t NodeId = {
    FCU = 0x00
    MCU = 0x01
}

enum_t Priority = {
    NORMAL = 0x00
    URGENT = 0x01
}

Byte Status = {
    StatusId = 0
    ErrorDetected = 1
    Reserved = ..
}

frame MockFrame {
    ID {
        NodeId | Priority | 0x00
    }
    byte Status
}
```

### 3. Use in Rust Code

```rust
use proto_c::include_cproto;

include_cproto!("protos/can.cproto");

fn main() {
    // Create a status byte with specific bits set
    let status = Status::new()
        .with_statusid(true)
        .with_errordetected(false);
    
    println!("Status byte: 0x{:02X}", status.to_byte());
    
    // Create a CAN frame
    let frame = MockFrame::new()
        .with_status(status);
    
    let can_frame = frame.to_can_frame(NodeId::FCU, Priority::URGENT);
    
    println!("CAN Frame:");
    println!("  ID: 0x{:08X}", can_frame.id);
    println!("  Data: {:?}", can_frame.data);
}
```

## Generated Code

The macro generates:

- **Enums** with `#[repr(u8)]` for each `enum_t` definition
- **Structs** for each `Byte` definition with:
  - Builder methods (`with_*`)
  - Getter methods for each bit field
  - `to_byte()` method for serialization
  - `from_byte()` method for deserialization
- **Frame structs** with:
  - Builder pattern for setting fields
  - `id()` method calculating the CAN ID
  - `to_can_frame()` method generating the final frame

## Example Output

```
Status byte: 0x01
CAN Frame:
  ID: 0x00000100
  Data: [1]
```

## License

MIT
