use crate::{KeyValue, ParseKeys, Variable};

#[allow(dead_code)]
pub fn p2s_case(s: String) -> String {
    let mut idx = 0usize;
    s.chars().enumerate().for_each(|(i, char)| {
       if i == 0 {
           if char.is_lowercase() {
               panic!("Pascal Case should be followed")
           }
           return
       }

       if char.is_uppercase() {
            idx = i;
       } 
    });
    //let (part1, part2) = s.split_at(idx);
    // part1.to_lowercase() + "_".into() + part2.to_lowercase()
    String::new()
}


pub fn enum_template(enum_vars: &KeyValue) -> String {
    let enum_name = &enum_vars.key;
    println!("Generating enum template for enum: '{}'", enum_name);
    println!("Enum variables: {:#?}", enum_vars.value);
    println!("Enum variables key: {}", enum_vars.key);
    let vars = enum_vars.value.iter().map(|e| {
        format!("{},", e.name)
    }).collect::<Vec<String>>().join("\n");
    format!("pub enum {} {{
            {}
        }}", enum_name, vars)
}

pub fn struct_template(struct_vars: &KeyValue, assign_val_to_type: bool) -> String {
    let struct_name = &struct_vars.key;
    println!("Generating struct template for struct: '{}'", struct_name);
    println!("Struct variables: {:#?}", struct_vars.value);
    println!("Struct variables key: {}", struct_vars.key);
    let vars = struct_vars.value.iter().map(|e| {
        format!("pub {}: {},", e.name, if !assign_val_to_type { "bool" } else { e.value.as_ref().unwrap() })
    }).collect::<Vec<String>>().join(ParseKeys::NewLine.into());
    format!("pub struct {} {{
            {}
        }}", struct_name, vars)
}

pub fn impl_u8_template(vars: &KeyValue, handler: &dyn Fn(&Variable) -> String) -> String {
    let name = &vars.key;
    println!("Generating impl template for enum: '{}'", name);
    println!("Impl variables: {:#?}", vars.value);
    println!("Impl variables key: {}", vars.key);
    let vars = vars.value.iter().map(handler).collect::<Vec<String>>().join(ParseKeys::NewLine.into());
    format!("impl From<{}> for u8 {{
            fn from(value: {}) -> Self {{
                match value {{
                    {}
                }}
            }}
        }}", name, name, vars)
}

pub fn impl_u8_slice_template(vars: &KeyValue, handler: &dyn Fn(&Variable) -> String) -> String {
    let name = &vars.key;
    println!("Generating impl template for enum: '{}'", name);
    println!("Impl variables: {:#?}", vars.value);
    println!("Impl variables key: {}", vars.key);
    let vars = vars.value.iter().map(handler).collect::<Vec<String>>().join(ParseKeys::NewLine.into());
    format!("impl From<{}> for &[u8] {{
            fn from(value: {}) -> Self {{
                match value {{
                    {}
                }}
            }}
        }}", name, name, vars)
}

#[inline]
pub fn trait_template() -> String {
    "trait Frame {
        pub fn can_id(&self) -> u32;
    }

    trait Parse {
        pub fn parse(&mut self);
    }".to_string()
}

pub fn impl_parse_template(vars: &KeyValue) -> String {
    let name = &vars.key;
    println!("Generating impl template for enum: '{}'", name);
    println!("Impl variables: {:#?}", vars.value);
    println!("Impl variables key: {}", vars.key);
    let vars = vars.value.iter().map(|e| {
        format!("self.{} = value & (0x01 << {}) != 0;", e.name, e.value.as_ref().unwrap())
    }).collect::<Vec<String>>().join(ParseKeys::NewLine.into());
    format!("impl {} {{
            pub fn parse(&mut self, value: u8) {{
                {}
            }}
        }}", name, vars)
}