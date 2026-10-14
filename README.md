# proto_c

Compiler for `.cp` CAN definitions. Generates Rust modules and C99 headers with typed payload structs, encode/decode functions, acceptance filters and RPC-style request/response helpers.

## Usage

```sh
cargo run -- protos/examples -o generated          # package -> generated/{rust,c}/examples/
cargo run -- protos/examples/dummy.cp -o generated  # one self-contained file (includes inlined)
cargo run -- --check protos/examples                # validate only
cargo run -- --lang c --c-out firmware/inc protos/examples
```

A directory is compiled as a **package**. `x.cp` becomes module/header `x` and `x.inc.cp` becomes `x_inc`. Rust also gets a `mod.rs` holding the shared prelude.

- **Rust:** `mod examples;`
- **C:** `#include "examples/dummy.h"`. Headers use only `static inline` functions, so no `.c` file is needed.

The compiler is also a library for use from `build.rs` or a proc-macro. `proto_c::compile(path)` takes a directory or a file and returns `Output::Package` or `Output::File`. `compile_package`, `compile_file` and `compile_str` are narrower variants.

## Language

`--` or `//` starts a comment. Bit 0 is the LSB.

### Directives and includes

```text
#protocol=can        -- optional; `#ignore=` skips the file
#inc=common          -- common.inc.cp from the same directory (nesting allowed, cycles rejected)
#endian=big          -- default byte order of this file (little if omitted)
```

`.inc.cp` files may contain `type`, `def`, `spec` and `filter`, but not `frame` or `fn`. Each is generated once and imported by the files that include it.

### Types and payload layout

```text
type Connection:            -- enum
.connected 1
.disconnected 0

def {1} ConnectionStatus:   -- bit fields pinned to payload byte(s)
.connected 0 Connection     -- typed field, width = bits the type needs

def {2,3} TemperatureError: -- `[#n]` selects the byte for the following fields
[#2]
.temp0 0                    -- single bit -> bool
.level 1..4                 -- bits 1..3 -> u8 (also `1..=3`, `4..`, `..` = whole byte)
[#3]
.temp8 0

def {4,5} Motor:
.rpm 0..12 i16 big          -- signed (sign-extended on decode), big-endian override
.dir 4 bool
```

A field's type can be an enum, `bool`, `u8`..`u64` or `i8`..`i64`. Without a type, 1 bit means `bool` and wider fields get the smallest fitting unsigned type. Enums are range-checked on decode.

**Byte order.** Bits are numbered LSB-first across the bytes a field spans, starting at its section byte. Those bytes are read as one little-endian number (default) or big-endian number (`big`, or `#endian=big`). With `big`, the section byte is the most significant byte. `[#0] .x 0..12 big` is the usual Motorola layout: high nibble in byte 0, low byte in byte 1. The file's byte order also applies to `.data` filter conditions and to standard-type fn parameters and return values.

### Spec (constants)

```text
spec:
.node_id 0x02 u8            -- standard types only: bool u8..u64 i8..i64 f32 f64
.gain 1.5                   -- type inferred when omitted
```

Use a spec value as `spec.node_id`.

### Filters

A filter matches raw bits of the CAN id and/or payload. Filters are independent of each other and can take parameters (standard types or enums):

```text
filter Node(node: NodeId):
.id 8..11 node              -- CAN id bits 8..10 (29-bit range)

filter Tag(tag: u8):
.data {0} 0..4 tag          -- payload byte 0, bits 0..3
```

Values can be literals, parameters, `spec.x` or `Type.Variant`.

Filters are combined with `|` at the point of use:

```text
frame CoprocStatus:
.size '4
.filter Node(spec.node_id) | Kind(Status) | Method(0)
.ConnectionStatus
.TemperatureError
```

The compiler rejects:
- combined filters that set a bit to two different values
- two filters in a package that could match the same frame
- payload fields that overlap bits a filter checks (the encoders set those bits automatically)

### Functions

```text
fn GetTemperature(): TemperatureError
.rsp Node(spec.node_id) | Kind(GetResponse) | Method(1)

fn Configure(motor: Motor, gain: f32, offset: i16): Void -- Void: no response
.fd                                                     -- allow payloads over 8 bytes
```

The caller builds and sends the request with a CAN id of their choosing; `.rsp` identifies the reply.

Parameters and return values can be standard types, `def`s or `frame`s:
- Defs and frames keep their fixed bytes.
- Standard types are packed little-endian into the next free bytes, skipping bytes that a `.rsp` filter checks.

## Generated API (sketch)

```rust
let bytes = SetFanSpeedRequest { value: 1200 }.to_bytes();           // send with your id
if GetTemperature::is_response(rx.id, rx.data) {
    let t: TemperatureError = GetTemperature::decode_response(rx.data)?;
}
let hw = (GetTemperature::RSP_ID_MASK, GetTemperature::RSP_ID_MATCH); // controller filter
```

```c
SetFanSpeedRequest req = { 1200 };
SetFanSpeedRequest_encode(&req, buf, sizeof buf);
if (GetTemperature_is_response(id, data, len))
    GetTemperature_decode_response(&temp, data, len);
```

## Layout

- `src/init.rs`: entry stage that loads a package or file, parses each file once, and links `#inc=` closures (missing files, cycles, ignored includes)
- `src/tools/parser.rs`: line parser that builds the AST (`ast.rs`)
- `src/tools/resolve.rs`: name resolution, validation, filter evaluation, payload layout (IR)
- `src/protocols/can.rs`: CAN payload limits
- `src/codegen/{rust,c}.rs`: code generators

## License

MIT
