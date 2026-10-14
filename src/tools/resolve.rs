//! Name resolution, validation and layout. Produces the IR consumed by code generators.

use std::collections::{HashMap, HashSet};

use crate::{error::ErrorSignals, protocols::can};

use super::{
    ast::{BitSpec, Cond, Def, FilterDef, FilterExpr, FnDef, Frame, Item, Literal, Param, Proto, Target, TypeDef, Value},
    syntax::{is_reserved, to_pascal, to_snake, to_upper_snake},
};

type Result<T> = std::result::Result<T, ErrorSignals>;

const MAX_ENUM_VALUE: u64 = i32::MAX as u64;
const ID_BITS: u32 = 29;
const MAX_PAYLOAD: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IntTy {
    U8,
    U16,
    U32,
    U64,
}

impl IntTy {
    pub fn for_bits(bits: u32) -> Self {
        match bits {
            0..=8 => Self::U8,
            9..=16 => Self::U16,
            17..=32 => Self::U32,
            _ => Self::U64,
        }
    }

    pub fn rust(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
        }
    }

    pub fn c(self) -> &'static str {
        match self {
            Self::U8 => "uint8_t",
            Self::U16 => "uint16_t",
            Self::U32 => "uint32_t",
            Self::U64 => "uint64_t",
        }
    }
}

/// Fixed-size standard types used by `spec`, filter parameters and fn parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Prim {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

impl Prim {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "bool" => Self::Bool,
            "u8" => Self::U8,
            "u16" => Self::U16,
            "u32" => Self::U32,
            "u64" => Self::U64,
            "i8" => Self::I8,
            "i16" => Self::I16,
            "i32" => Self::I32,
            "i64" => Self::I64,
            "f32" => Self::F32,
            "f64" => Self::F64,
            _ => return None,
        })
    }

    pub fn size(self) -> usize {
        match self {
            Self::Bool | Self::U8 | Self::I8 => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
        }
    }

    pub fn rust(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::F32 => "f32",
            Self::F64 => "f64",
        }
    }

    pub fn c(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::U8 => "uint8_t",
            Self::U16 => "uint16_t",
            Self::U32 => "uint32_t",
            Self::U64 => "uint64_t",
            Self::I8 => "int8_t",
            Self::I16 => "int16_t",
            Self::I32 => "int32_t",
            Self::I64 => "int64_t",
            Self::F32 => "float",
            Self::F64 => "double",
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Self::F32 | Self::F64)
    }

    pub fn is_signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    fn int_range(self) -> Option<(i128, i128)> {
        let bits = self.size() as u32 * 8;
        match self {
            Self::U8 | Self::U16 | Self::U32 | Self::U64 => Some((0, (1i128 << bits) - 1)),
            Self::I8 | Self::I16 | Self::I32 | Self::I64 => Some((-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)),
            _ => None,
        }
    }
}

/// Resolved scope of one file. Items marked `external` come from includes and are generated there.
#[derive(Debug, Default)]
pub struct Model {
    pub enums: Vec<EnumIr>,
    pub defs: Vec<DefIr>,
    pub specs: Vec<SpecIr>,
    pub frames: Vec<FrameIr>,
    pub fns: Vec<FnIr>,
}

impl Model {
    /// Every filter this file applies, labelled for diagnostics.
    pub fn filter_uses(&self) -> Vec<(String, &FilterIr)> {
        let frames = self.frames.iter().filter(|f| !f.external).filter_map(|f| {
            f.filter.as_ref().map(|flt| (format!("frame `{}` .filter", f.name), flt))
        });
        let fns = self.fns.iter().filter(|f| !f.external).filter_map(|f| {
            f.rsp.as_ref().map(|flt| (format!("fn `{}` .rsp", f.name), flt))
        });
        frames.chain(fns).collect()
    }
}

#[derive(Debug)]
pub struct EnumIr {
    pub name: String,
    pub repr: IntTy,
    /// Bits needed for the largest variant value.
    pub width: u32,
    pub variants: Vec<(String, u64)>,
    /// Index of the variant used for `Default` (the one with value 0, else the first).
    pub default: usize,
    pub external: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum FieldTy {
    Bool,
    Uint(IntTy),
    Enum(usize),
}

#[derive(Debug)]
pub struct FieldIr {
    pub name: String,
    /// Absolute bit position in the payload (`byte * 8 + bit`, bit 0 = LSB).
    pub pos: usize,
    pub width: u32,
    pub ty: FieldTy,
}

/// A slice of a field that lives in a single payload byte.
#[derive(Debug, PartialEq)]
pub struct Chunk {
    pub byte: usize,
    /// Bit offset inside the byte.
    pub shift: u32,
    /// Unshifted mask of the chunk's bits.
    pub mask: u8,
    /// Offset of this chunk inside the field value.
    pub vshift: u32,
}

impl FieldIr {
    /// Splits the field into per-byte chunks, little-endian (low value bits in the lowest byte).
    pub fn chunks(&self) -> Vec<Chunk> {
        let mut out = Vec::new();
        let (mut pos, end) = (self.pos, self.pos + self.width as usize);
        while pos < end {
            let shift = (pos % 8) as u32;
            let n = (8 - shift).min((end - pos) as u32);
            out.push(Chunk {
                byte: pos / 8,
                shift,
                mask: (((1u16 << n) - 1) & 0xFF) as u8,
                vshift: (pos - self.pos) as u32,
            });
            pos += n as usize;
        }
        out
    }
}

#[derive(Debug)]
pub struct DefIr {
    pub name: String,
    pub bytes: Vec<usize>,
    /// Minimum payload length needed to hold this def.
    pub min_len: usize,
    pub fields: Vec<FieldIr>,
    pub external: bool,
}

#[derive(Debug)]
pub struct SpecIr {
    pub name: String,
    pub ty: Prim,
    pub value: Literal,
    pub external: bool,
}

/// Mask/match over the CAN id and payload. Data vectors are trimmed to the last checked byte.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterIr {
    pub id_mask: u32,
    pub id_match: u32,
    pub data_mask: Vec<u8>,
    pub data_match: Vec<u8>,
}

impl FilterIr {
    pub fn data_len(&self) -> usize {
        self.data_mask.len()
    }

    /// True if some frame could pass both filters.
    pub fn overlaps(&self, o: &Self) -> bool {
        if (self.id_mask & o.id_mask) & (self.id_match ^ o.id_match) != 0 {
            return false;
        }
        (0..self.data_len().min(o.data_len()))
            .all(|i| (self.data_mask[i] & o.data_mask[i]) & (self.data_match[i] ^ o.data_match[i]) == 0)
    }

    fn data_bit(&self, pos: usize) -> bool {
        self.data_mask.get(pos / 8).is_some_and(|m| m >> (pos % 8) & 1 != 0)
    }
}

#[derive(Debug)]
pub struct FrameIr {
    pub name: String,
    pub size: usize,
    pub fd: bool,
    /// (struct field name, index into `Model::defs`)
    pub members: Vec<(String, usize)>,
    pub filter: Option<FilterIr>,
    pub external: bool,
}

/// Where a fn parameter or return value lives in the payload.
#[derive(Debug, Clone, Copy)]
pub enum Slot {
    /// Little-endian at a byte offset.
    Prim(Prim, usize),
    Def(usize),
    Frame(usize),
}

#[derive(Debug)]
pub struct ParamIr {
    pub name: String,
    pub slot: Slot,
}

#[derive(Debug)]
pub struct FnIr {
    pub name: String,
    pub params: Vec<ParamIr>,
    pub req_size: usize,
    pub ret: Option<Slot>,
    pub rsp: Option<FilterIr>,
    pub rsp_size: usize,
    pub fd: bool,
    pub external: bool,
}

pub fn check_overlaps(uses: &[(String, &FilterIr)]) -> Result<()> {
    for (i, (a, fa)) in uses.iter().enumerate() {
        for (b, fb) in &uses[i + 1..] {
            if fa.overlaps(fb) {
                return Err(ErrorSignals::Generic(format!("{a} and {b} can match the same frame")));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Sym<'a> {
    Enum(usize),
    Def(usize),
    Frame(usize),
    Filter(&'a FilterDef),
    Other,
}

#[derive(Debug, Clone, Copy)]
enum Val {
    Int(i128),
    Float(f64),
    Bool(bool),
}

impl From<Literal> for Val {
    fn from(l: Literal) -> Self {
        match l {
            Literal::Int(v) => Val::Int(v),
            Literal::Float(v) => Val::Float(v),
            Literal::Bool(v) => Val::Bool(v),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum ParamTy {
    Prim(Prim),
    Enum(usize),
}

struct Ctx<'a> {
    syms: HashMap<&'a str, Sym<'a>>,
    specs: HashMap<&'a str, Val>,
    model: Model,
}

fn check_name(name: &str, ln: usize) -> Result<()> {
    if is_reserved(name) {
        return Err(ErrorSignals::at(ln, format!("`{name}` is a reserved word in the generated code")));
    }
    Ok(())
}

/// Resolves a file together with its includes; the bool marks files whose items are external.
pub fn resolve(scope: &[(&Proto, bool)]) -> Result<Model> {
    let items: Vec<(&Item, bool)> =
        scope.iter().flat_map(|(p, ext)| p.items.iter().map(move |i| (i, *ext))).collect();

    let mut ctx = Ctx { syms: HashMap::new(), specs: HashMap::new(), model: Model::default() };
    let mut c_names: HashMap<String, usize> = HashMap::new();
    let (mut n_enum, mut n_def, mut n_frame) = (0, 0, 0);
    for (item, _) in &items {
        let Some(name) = item.name() else { continue };
        let ln = item.line();
        check_name(name, ln)?;
        let sym = match item {
            Item::Type(_) => Sym::Enum(post_inc(&mut n_enum)),
            Item::Def(_) => Sym::Def(post_inc(&mut n_def)),
            Item::Frame(_) => Sym::Frame(post_inc(&mut n_frame)),
            Item::Filter(f) => Sym::Filter(f),
            _ => Sym::Other,
        };
        let mut names = vec![name.to_string()];
        if let Item::Fn(_) = item {
            names.push(format!("{name}Request"));
        }
        for n in names {
            if let Some(prev) = c_names.insert(to_upper_snake(&n), ln) {
                return Err(ErrorSignals::at(ln, format!("`{n}` clashes with the name defined on line {prev}")));
            }
        }
        if ctx.syms.insert(name, sym).is_some() {
            return Err(ErrorSignals::at(ln, format!("`{name}` is defined more than once")));
        }
    }

    for (item, ext) in &items {
        if let Item::Type(t) = item {
            ctx.model.enums.push(resolve_enum(t, *ext)?);
        }
    }
    for (item, ext) in &items {
        let Item::Spec(s) = item else { continue };
        for e in &s.entries {
            check_name(&e.name, e.line)?;
            let ty = match &e.ty {
                Some(t) => Prim::parse(t)
                    .ok_or_else(|| ErrorSignals::at(e.line, format!("spec values must use a standard type, found `{t}`")))?,
                None => infer_prim(e.value),
            };
            let val = coerce(e.value.into(), ParamTy::Prim(ty), &ctx.model).map_err(|m| ErrorSignals::at(e.line, m))?;
            if ctx.specs.insert(&e.name, val).is_some() {
                return Err(ErrorSignals::at(e.line, format!("`spec.{}` is defined more than once", e.name)));
            }
            let value = match val {
                Val::Int(v) => Literal::Int(v),
                Val::Float(v) => Literal::Float(v),
                Val::Bool(v) => Literal::Bool(v),
            };
            ctx.model.specs.push(SpecIr { name: e.name.clone(), ty, value, external: *ext });
        }
    }
    for (item, ext) in &items {
        if let Item::Def(d) = item {
            let def = resolve_def(d, *ext, &ctx)?;
            ctx.model.defs.push(def);
        }
    }
    for (item, ext) in &items {
        if let (Item::Filter(f), false) = (item, ext) {
            validate_filter(f, &ctx)?;
        }
    }
    for (item, ext) in &items {
        if let Item::Frame(fr) = item {
            let frame = resolve_frame(fr, *ext, &ctx)?;
            ctx.model.frames.push(frame);
        }
    }
    for (item, ext) in &items {
        if let Item::Fn(f) = item {
            let func = resolve_fn(f, *ext, &ctx)?;
            ctx.model.fns.push(func);
        }
    }
    Ok(ctx.model)
}

fn infer_prim(l: Literal) -> Prim {
    match l {
        Literal::Bool(_) => Prim::Bool,
        Literal::Float(_) => Prim::F32,
        Literal::Int(v) if v >= 0 && v <= i128::from(u32::MAX) => Prim::U32,
        Literal::Int(v) if v >= 0 => Prim::U64,
        Literal::Int(v) if v >= i128::from(i32::MIN) => Prim::I32,
        Literal::Int(_) => Prim::I64,
    }
}

fn resolve_enum(t: &TypeDef, external: bool) -> Result<EnumIr> {
    if t.variants.is_empty() {
        return Err(ErrorSignals::at(t.line, format!("type `{}` has no variants", t.name)));
    }
    let (mut names, mut values) = (HashSet::new(), HashMap::new());
    for v in &t.variants {
        check_name(&v.name, v.line)?;
        if !names.insert(to_pascal(&v.name)) {
            return Err(ErrorSignals::at(v.line, format!("duplicate variant `{}`", v.name)));
        }
        if let Some(other) = values.insert(v.value, &v.name) {
            return Err(ErrorSignals::at(v.line, format!("`{}` has the same value as `{other}`", v.name)));
        }
        if v.value > MAX_ENUM_VALUE {
            return Err(ErrorSignals::at(v.line, format!("enum value must be <= {MAX_ENUM_VALUE}")));
        }
    }
    let width = bit_width(t.variants.iter().map(|v| v.value).max().unwrap_or(0));
    Ok(EnumIr {
        name: t.name.clone(),
        repr: IntTy::for_bits(width),
        width,
        variants: t.variants.iter().map(|v| (v.name.clone(), v.value)).collect(),
        default: t.variants.iter().position(|v| v.value == 0).unwrap_or(0),
        external,
    })
}

fn lookup_enum(ctx: &Ctx, ty: &str, ln: usize) -> Result<usize> {
    match ctx.syms.get(ty) {
        Some(Sym::Enum(i)) => Ok(*i),
        Some(_) => Err(ErrorSignals::at(ln, format!("`{ty}` is not a `type`"))),
        None => Err(ErrorSignals::at(ln, format!("unknown type `{ty}`"))),
    }
}

fn resolve_def(d: &Def, external: bool, ctx: &Ctx) -> Result<DefIr> {
    let mut seen_bytes = HashSet::new();
    if let Some(b) = d.bytes.iter().find(|b| !seen_bytes.insert(**b)) {
        return Err(ErrorSignals::at(d.line, format!("byte {b} listed twice")));
    }
    if let Some(b) = d.bytes.iter().find(|b| **b >= MAX_PAYLOAD) {
        return Err(ErrorSignals::at(d.line, format!("byte {b} is beyond the {MAX_PAYLOAD}-byte CAN FD payload")));
    }
    if d.fields.is_empty() {
        return Err(ErrorSignals::at(d.line, format!("def `{}` has no fields", d.name)));
    }
    let mut used_bits: HashMap<usize, &str> = HashMap::new();
    let mut names = HashSet::new();
    let mut fields = Vec::new();
    for f in &d.fields {
        check_name(&f.name, f.line)?;
        if !names.insert(f.name.as_str()) {
            return Err(ErrorSignals::at(f.line, format!("duplicate field `{}`", f.name)));
        }
        let enum_ty = f.ty.as_deref().map(|ty| lookup_enum(ctx, ty, f.line)).transpose()?;
        let enum_width = enum_ty.map(|i| ctx.model.enums[i].width);
        let (start, width) = match f.bits {
            BitSpec::Full => (0, 8),
            BitSpec::Bit(n) => (n, enum_width.unwrap_or(1)),
            BitSpec::From(n) => (n, 8u32.saturating_sub(n)),
            BitSpec::Range(a, b) => (a, b - a),
        };
        if start >= 8 {
            return Err(ErrorSignals::at(f.line, format!("start bit {start} is outside the byte (0..8)")));
        }
        if width > 64 {
            return Err(ErrorSignals::at(f.line, format!("field is {width} bits wide, max is 64")));
        }
        if let Some(ew) = enum_width.filter(|ew| *ew > width) {
            return Err(ErrorSignals::at(
                f.line,
                format!("`{}` needs {ew} bits but the field has {width}", f.ty.as_deref().unwrap_or_default()),
            ));
        }
        let pos = f.byte * 8 + start as usize;
        for bit in pos..pos + width as usize {
            if !d.bytes.contains(&(bit / 8)) {
                return Err(ErrorSignals::at(
                    f.line,
                    format!("field `{}` reaches byte {} which is not declared in `{}`", f.name, bit / 8, d.name),
                ));
            }
            if let Some(other) = used_bits.insert(bit, &f.name) {
                return Err(ErrorSignals::at(
                    f.line,
                    format!("field `{}` overlaps `{other}` at byte {} bit {}", f.name, bit / 8, bit % 8),
                ));
            }
        }
        let ty = match enum_ty {
            Some(i) => FieldTy::Enum(i),
            None if width == 1 => FieldTy::Bool,
            None => FieldTy::Uint(IntTy::for_bits(width)),
        };
        fields.push(FieldIr { name: f.name.clone(), pos, width, ty });
    }
    Ok(DefIr {
        name: d.name.clone(),
        bytes: d.bytes.clone(),
        min_len: d.bytes.iter().max().map_or(0, |m| m + 1),
        fields,
        external,
    })
}

// ---- filters ----

/// Mask/match accumulator for one filter expression.
struct Acc {
    id_mask: u32,
    id_match: u32,
    data_mask: [u8; MAX_PAYLOAD],
    data_match: [u8; MAX_PAYLOAD],
}

impl Acc {
    fn new() -> Self {
        Self { id_mask: 0, id_match: 0, data_mask: [0; MAX_PAYLOAD], data_match: [0; MAX_PAYLOAD] }
    }

    fn set(&mut self, target: Target, start: usize, width: u32, pattern: u64) -> std::result::Result<(), String> {
        for i in 0..width as usize {
            let want = (pattern >> i) & 1 != 0;
            let pos = start + i;
            let (set, cur, what) = match target {
                Target::Id => (self.id_mask >> pos & 1 != 0, self.id_match >> pos & 1 != 0, format!("id bit {pos}")),
                Target::Data(_) => (
                    self.data_mask[pos / 8] >> (pos % 8) & 1 != 0,
                    self.data_match[pos / 8] >> (pos % 8) & 1 != 0,
                    format!("data byte {} bit {}", pos / 8, pos % 8),
                ),
            };
            if set && cur != want {
                return Err(format!("conflicting values for {what}"));
            }
            match target {
                Target::Id => {
                    self.id_mask |= 1 << pos;
                    self.id_match |= u32::from(want) << pos;
                }
                Target::Data(_) => {
                    self.data_mask[pos / 8] |= 1 << (pos % 8);
                    self.data_match[pos / 8] |= u8::from(want) << (pos % 8);
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> FilterIr {
        let len = self.data_mask.iter().rposition(|m| *m != 0).map_or(0, |i| i + 1);
        FilterIr {
            id_mask: self.id_mask,
            id_match: self.id_match,
            data_mask: self.data_mask[..len].to_vec(),
            data_match: self.data_match[..len].to_vec(),
        }
    }
}

/// Absolute start bit and width of a condition (id bit, or payload bit for data).
fn cond_span(c: &Cond) -> std::result::Result<(usize, u32), String> {
    match c.target {
        Target::Id => {
            let (start, width) = match c.bits {
                BitSpec::Full => (0, ID_BITS),
                BitSpec::Bit(n) => (n, 1),
                BitSpec::From(n) => (n, ID_BITS.saturating_sub(n)),
                BitSpec::Range(a, b) => (a, b - a),
            };
            if width == 0 || start + width > ID_BITS {
                return Err(format!("id bits must be within 0..{ID_BITS}"));
            }
            Ok((start as usize, width))
        }
        Target::Data(byte) => {
            let (start, width) = match c.bits {
                BitSpec::Full => (0, 8),
                BitSpec::Bit(n) => (n, 1),
                BitSpec::From(n) => (n, 8u32.saturating_sub(n)),
                BitSpec::Range(a, b) => (a, b - a),
            };
            if start >= 8 || width == 0 {
                return Err(format!("start bit {start} is outside the byte (0..8)"));
            }
            if width > 64 {
                return Err(format!("condition is {width} bits wide, max is 64"));
            }
            let pos = byte * 8 + start as usize;
            if pos + width as usize > MAX_PAYLOAD * 8 {
                return Err(format!("data bits reach beyond the {MAX_PAYLOAD}-byte payload"));
            }
            Ok((pos, width))
        }
    }
}

fn param_ty(p: &Param, ctx: &Ctx, ln: usize) -> Result<ParamTy> {
    if let Some(prim) = Prim::parse(&p.ty) {
        return Ok(ParamTy::Prim(prim));
    }
    lookup_enum(ctx, &p.ty, ln).map(ParamTy::Enum)
}

fn eval(v: &Value, env: &HashMap<&str, Val>, expect: Option<ParamTy>, ctx: &Ctx) -> std::result::Result<Val, String> {
    match v {
        Value::Lit(l) => Ok((*l).into()),
        Value::Spec(n) => ctx.specs.get(n.as_str()).copied().ok_or_else(|| format!("unknown `spec.{n}`")),
        Value::Variant(t, n) => {
            let Some(Sym::Enum(i)) = ctx.syms.get(t.as_str()) else {
                return Err(format!("unknown type `{t}`"));
            };
            variant_value(&ctx.model.enums[*i], n).ok_or_else(|| format!("`{t}` has no variant `{n}`"))
        }
        Value::Name(n) => {
            if let Some(v) = env.get(n.as_str()) {
                return Ok(*v);
            }
            if let Some(ParamTy::Enum(i)) = expect {
                let e = &ctx.model.enums[i];
                return variant_value(e, n).ok_or_else(|| format!("`{}` has no variant `{n}`", e.name));
            }
            Err(format!("unknown name `{n}`"))
        }
    }
}

fn variant_value(e: &EnumIr, name: &str) -> Option<Val> {
    e.variants.iter().find(|(v, _)| v == name).map(|(_, val)| Val::Int(i128::from(*val)))
}

fn coerce(v: Val, ty: ParamTy, model: &Model) -> std::result::Result<Val, String> {
    match (ty, v) {
        (ParamTy::Enum(i), Val::Int(n)) => {
            let e = &model.enums[i];
            if e.variants.iter().any(|(_, val)| i128::from(*val) == n) {
                Ok(v)
            } else {
                Err(format!("{n} is not a value of `{}`", e.name))
            }
        }
        (ParamTy::Prim(Prim::Bool), Val::Bool(_)) => Ok(v),
        (ParamTy::Prim(Prim::F32), Val::Float(f)) => Ok(Val::Float(f64::from(f as f32))),
        (ParamTy::Prim(Prim::F32), Val::Int(n)) => Ok(Val::Float(f64::from(n as f32))),
        (ParamTy::Prim(Prim::F64), Val::Float(_)) => Ok(v),
        (ParamTy::Prim(Prim::F64), Val::Int(n)) => Ok(Val::Float(n as f64)),
        (ParamTy::Prim(p), Val::Int(n)) if !p.is_float() && p != Prim::Bool => {
            let (lo, hi) = p.int_range().unwrap_or((0, 0));
            if (lo..=hi).contains(&n) {
                Ok(v)
            } else {
                Err(format!("{n} does not fit `{}`", p.rust()))
            }
        }
        (ParamTy::Enum(i), _) => Err(format!("expected a `{}` value", model.enums[i].name)),
        (ParamTy::Prim(p), _) => Err(format!("expected a `{}` value", p.rust())),
    }
}

/// Bit pattern of `v` in a `width`-bit field.
fn pattern(v: Val, width: u32) -> std::result::Result<u64, String> {
    let max = if width >= 64 { u64::MAX } else { (1u64 << width) - 1 };
    match v {
        Val::Bool(b) => Ok(u64::from(b)),
        Val::Int(n) if n >= 0 => {
            if n > i128::from(max) {
                return Err(format!("{n} does not fit in {width} bits"));
            }
            Ok(n as u64)
        }
        Val::Int(n) => {
            if width < 128 && n < -(1i128 << (width - 1)) {
                return Err(format!("{n} does not fit in {width} signed bits"));
            }
            Ok((n as i64 as u64) & max)
        }
        Val::Float(f) => match width {
            32 => Ok(u64::from((f as f32).to_bits())),
            64 => Ok(f.to_bits()),
            _ => Err(format!("float values need a 32 or 64 bit field, not {width}")),
        },
    }
}

/// Applies a filter's conditions; errors carry the condition's line.
fn apply(f: &FilterDef, args: &[Val], acc: &mut Acc, ctx: &Ctx) -> std::result::Result<(), (usize, String)> {
    let env: HashMap<&str, Val> = f.params.iter().map(|p| p.name.as_str()).zip(args.iter().copied()).collect();
    for c in &f.conds {
        let (start, width) = cond_span(c).map_err(|e| (c.line, e))?;
        let val = eval(&c.value, &env, None, ctx).map_err(|e| (c.line, e))?;
        let pat = pattern(val, width).map_err(|e| (c.line, e))?;
        acc.set(c.target, start, width, pat).map_err(|e| (c.line, e))?;
    }
    Ok(())
}

fn validate_filter(f: &FilterDef, ctx: &Ctx) -> Result<()> {
    if f.conds.is_empty() {
        return Err(ErrorSignals::at(f.line, format!("filter `{}` has no conditions", f.name)));
    }
    let mut names = HashSet::new();
    let mut placeholders = Vec::new();
    for p in &f.params {
        check_name(&p.name, f.line)?;
        if !names.insert(p.name.as_str()) {
            return Err(ErrorSignals::at(f.line, format!("duplicate parameter `{}`", p.name)));
        }
        placeholders.push(match param_ty(p, ctx, f.line)? {
            ParamTy::Prim(Prim::Bool) => Val::Bool(false),
            ParamTy::Prim(p) if p.is_float() => Val::Float(0.0),
            _ => Val::Int(0),
        });
    }
    apply(f, &placeholders, &mut Acc::new(), ctx).map_err(|(ln, e)| ErrorSignals::at(ln, e))
}

fn eval_expr(expr: &FilterExpr, ctx: &Ctx) -> Result<FilterIr> {
    let ln = expr.line;
    let mut acc = Acc::new();
    for call in &expr.calls {
        let f = match ctx.syms.get(call.name.as_str()) {
            Some(Sym::Filter(f)) => *f,
            Some(_) => return Err(ErrorSignals::at(ln, format!("`{}` is not a filter", call.name))),
            None => return Err(ErrorSignals::at(ln, format!("unknown filter `{}`", call.name))),
        };
        if call.args.len() != f.params.len() {
            return Err(ErrorSignals::at(
                ln,
                format!("filter `{}` takes {} argument(s), got {}", f.name, f.params.len(), call.args.len()),
            ));
        }
        let mut args = Vec::new();
        for (a, p) in call.args.iter().zip(&f.params) {
            let ty = param_ty(p, ctx, ln)?;
            let v = eval(a, &HashMap::new(), Some(ty), ctx)
                .and_then(|v| coerce(v, ty, &ctx.model))
                .map_err(|e| ErrorSignals::at(ln, format!("argument `{}` of `{}`: {e}", p.name, f.name)))?;
            args.push(v);
        }
        apply(f, &args, &mut acc, ctx)
            .map_err(|(_, e)| ErrorSignals::at(ln, format!("in filter `{}`: {e}", f.name)))?;
    }
    Ok(acc.finish())
}

/// Bits written by a def or frame (fields and, for frames, their own filter).
fn slot_bits(slot: Slot, model: &Model) -> Vec<usize> {
    let def_bits = |d: usize| model.defs[d].fields.iter().flat_map(|f| f.pos..f.pos + f.width as usize).collect::<Vec<_>>();
    match slot {
        Slot::Def(d) => def_bits(d),
        Slot::Frame(fr) => model.frames[fr].members.iter().flat_map(|(_, d)| def_bits(*d)).collect(),
        Slot::Prim(p, off) => (off * 8..(off + p.size()) * 8).collect(),
    }
}

fn check_reserved(filter: &FilterIr, slot: Slot, model: &Model, ln: usize, what: &str) -> Result<()> {
    if let Some(bit) = slot_bits(slot, model).into_iter().find(|b| filter.data_bit(*b)) {
        return Err(ErrorSignals::at(
            ln,
            format!("{what} uses data byte {} bit {} which the filter reserves", bit / 8, bit % 8),
        ));
    }
    if let Slot::Frame(fr) = slot {
        if let Some(own) = &model.frames[fr].filter {
            for i in 0..filter.data_len().min(own.data_len()) {
                if (filter.data_mask[i] & own.data_mask[i]) & (filter.data_match[i] ^ own.data_match[i]) != 0 {
                    return Err(ErrorSignals::at(ln, format!("filter contradicts the data bits of `{}`", model.frames[fr].name)));
                }
            }
        }
    }
    Ok(())
}

fn resolve_frame(fr: &Frame, external: bool, ctx: &Ctx) -> Result<FrameIr> {
    if fr.members.is_empty() {
        return Err(ErrorSignals::at(fr.line, format!("frame `{}` has no members", fr.name)));
    }
    let mut owner: HashMap<usize, &str> = HashMap::new();
    let mut members = Vec::new();
    let mut field_names = HashSet::new();
    for (name, ln) in &fr.members {
        let idx = match ctx.syms.get(name.as_str()) {
            Some(Sym::Def(i)) => *i,
            Some(_) => return Err(ErrorSignals::at(*ln, format!("`{name}` is not a `def`"))),
            None => return Err(ErrorSignals::at(*ln, format!("unknown def `{name}`"))),
        };
        let field = to_snake(name);
        if !field_names.insert(field.clone()) {
            return Err(ErrorSignals::at(*ln, format!("`{name}` is included more than once")));
        }
        for &b in &ctx.model.defs[idx].bytes {
            if let Some(other) = owner.insert(b, name) {
                return Err(ErrorSignals::at(*ln, format!("`{name}` and `{other}` both use byte {b}")));
            }
        }
        members.push((field, idx));
    }
    let filter = fr.filter.as_ref().map(|e| eval_expr(e, ctx)).transpose()?;
    if let Some(flt) = &filter {
        for (_, d) in &members {
            check_reserved(flt, Slot::Def(*d), &ctx.model, fr.line, &format!("`{}`", ctx.model.defs[*d].name))?;
        }
    }
    let used = owner.keys().max().map_or(0, |m| m + 1).max(filter.as_ref().map_or(0, FilterIr::data_len));
    let size = fr.size.unwrap_or(used);
    if used > size {
        return Err(ErrorSignals::at(fr.line, format!("members and filter use {used} bytes but `.size` is {size}")));
    }
    can::check_size(size, fr.fd).map_err(|e| ErrorSignals::at(fr.line, e))?;
    Ok(FrameIr { name: fr.name.clone(), size, fd: fr.fd, members, filter, external })
}

/// Lowest offset >= `from` with `size` free bytes.
fn place(used: &[bool; MAX_PAYLOAD], from: usize, size: usize) -> Option<usize> {
    (from..=MAX_PAYLOAD.saturating_sub(size)).find(|o| used[*o..o + size].iter().all(|u| !u))
}

fn fixed_slot(ty: &str, ctx: &Ctx, ln: usize) -> Result<Option<Slot>> {
    Ok(match ctx.syms.get(ty) {
        Some(Sym::Def(d)) => Some(Slot::Def(*d)),
        Some(Sym::Frame(f)) => Some(Slot::Frame(*f)),
        Some(Sym::Enum(_)) => return Err(ErrorSignals::at(ln, format!("enum `{ty}` cannot be sent directly; wrap it in a `def`"))),
        Some(_) => return Err(ErrorSignals::at(ln, format!("`{ty}` is not a type"))),
        None if Prim::parse(ty).is_some() => None,
        None => {
            let hint = match ty {
                "int" => " (use i8/i16/i32/i64 or u8..u64)",
                "float" => " (use f32 or f64)",
                "string" => " (strings are not supported in CAN payloads)",
                _ => "",
            };
            return Err(ErrorSignals::at(ln, format!("unknown type `{ty}`{hint}")));
        }
    })
}

fn slot_bytes(slot: Slot, model: &Model) -> Vec<usize> {
    match slot {
        Slot::Def(d) => model.defs[d].bytes.clone(),
        Slot::Frame(f) => (0..model.frames[f].size).collect(),
        Slot::Prim(p, off) => (off..off + p.size()).collect(),
    }
}

fn finish_size(size: usize, fd: bool, ln: usize, what: &str) -> Result<usize> {
    if fd {
        can::fd_len(size).ok_or_else(|| ErrorSignals::at(ln, format!("{what} needs {size} bytes, more than CAN FD allows")))
    } else if size > can::MAX_CLASSIC_LEN {
        Err(ErrorSignals::at(ln, format!("{what} needs {size} bytes; add `.fd` for CAN FD")))
    } else {
        Ok(size)
    }
}

fn resolve_fn(f: &FnDef, external: bool, ctx: &Ctx) -> Result<FnIr> {
    let ln = f.line;
    let mut names = HashSet::new();
    let mut used = [false; MAX_PAYLOAD];
    let mut slots: Vec<Option<Slot>> = Vec::new();
    for p in &f.params {
        check_name(&p.name, ln)?;
        if !names.insert(p.name.as_str()) {
            return Err(ErrorSignals::at(ln, format!("duplicate parameter `{}`", p.name)));
        }
        let slot = fixed_slot(&p.ty, ctx, ln)?;
        if let Some(slot) = slot {
            for b in slot_bytes(slot, &ctx.model) {
                if std::mem::replace(&mut used[b], true) {
                    return Err(ErrorSignals::at(ln, format!("parameter `{}` overlaps another at byte {b}", p.name)));
                }
            }
        }
        slots.push(slot);
    }
    let mut cursor = 0;
    let mut params = Vec::new();
    for (p, slot) in f.params.iter().zip(slots) {
        let slot = match slot {
            Some(s) => s,
            None => {
                let prim = Prim::parse(&p.ty).expect("checked by fixed_slot");
                let off = place(&used, cursor, prim.size())
                    .ok_or_else(|| ErrorSignals::at(ln, format!("no room for parameter `{}`", p.name)))?;
                used[off..off + prim.size()].iter_mut().for_each(|u| *u = true);
                cursor = off + prim.size();
                Slot::Prim(prim, off)
            }
        };
        params.push(ParamIr { name: p.name.clone(), slot });
    }
    let req_end = used.iter().rposition(|u| *u).map_or(0, |i| i + 1);
    let req_size = finish_size(req_end, f.fd, ln, "request")?;

    let (ret, rsp, rsp_size) = match (&f.ret, &f.rsp) {
        (None, None) => (None, None, 0),
        (None, Some(e)) => return Err(ErrorSignals::at(e.line, "`Void` functions have no response to filter")),
        (Some(_), None) => return Err(ErrorSignals::at(ln, format!("fn `{}` returns a value and needs `.rsp`", f.name))),
        (Some(ty), Some(e)) => {
            let filter = eval_expr(e, ctx)?;
            let slot = match fixed_slot(ty, ctx, ln)? {
                Some(s) => s,
                None => {
                    let prim = Prim::parse(ty).expect("checked by fixed_slot");
                    let mut reserved = [false; MAX_PAYLOAD];
                    filter.data_mask.iter().enumerate().for_each(|(i, m)| reserved[i] = *m != 0);
                    let off = place(&reserved, 0, prim.size())
                        .ok_or_else(|| ErrorSignals::at(ln, "no room for the return value"))?;
                    Slot::Prim(prim, off)
                }
            };
            check_reserved(&filter, slot, &ctx.model, e.line, "return value")?;
            let end = slot_bytes(slot, &ctx.model).into_iter().max().map_or(0, |b| b + 1).max(filter.data_len());
            (Some(slot), Some(filter), finish_size(end, f.fd, ln, "response")?)
        }
    };
    Ok(FnIr { name: f.name.clone(), params, req_size, ret, rsp, rsp_size, fd: f.fd, external })
}

fn post_inc(n: &mut usize) -> usize {
    *n += 1;
    *n - 1
}

fn bit_width(v: u64) -> u32 {
    (64 - v.leading_zeros()).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_span_bytes() {
        let f = FieldIr { name: "x".into(), pos: 2 * 8 + 4, width: 10, ty: FieldTy::Uint(IntTy::U16) };
        assert_eq!(
            f.chunks(),
            vec![
                Chunk { byte: 2, shift: 4, mask: 0x0F, vshift: 0 },
                Chunk { byte: 3, shift: 0, mask: 0x3F, vshift: 4 },
            ]
        );
    }

    #[test]
    fn overlap() {
        let a = FilterIr { id_mask: 0x700, id_match: 0x200, ..Default::default() };
        let b = FilterIr { id_mask: 0x707, id_match: 0x201, ..Default::default() };
        let c = FilterIr { id_mask: 0x700, id_match: 0x300, ..Default::default() };
        assert!(a.overlaps(&b));
        assert!(!a.overlaps(&c));
    }

    #[test]
    fn patterns() {
        assert_eq!(pattern(Val::Int(-1), 3), Ok(0b111));
        assert!(pattern(Val::Int(8), 3).is_err());
        assert_eq!(pattern(Val::Float(1.0), 32), Ok(u64::from(1.0f32.to_bits())));
    }
}
