
mod example {
    use proto_c::include_can_proto;
    include_can_proto!("protos/dummy.cproto");
}

use example::*;

fn main() {
    println!("Hello, world!");
    println!("{:#?}", NodeId::MCU);

    let mut status_can= Status::default();

    status_can.parse(0b1000_0001);

    println!("{:#?}", status_can);
}