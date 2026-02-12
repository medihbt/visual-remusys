use remusys_ir::base::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use smol_str::{ToSmolStr, format_smolstr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct I64Codec(pub i64);

impl Serialize for I64Codec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_smolstr())
    }
}
impl<'de> Deserialize<'de> for I64Codec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        match s.parse::<i64>() {
            Ok(v) => Ok(I64Codec(v)),
            Err(_) => Err(serde::de::Error::custom(format!("invalid i64 '{s}'"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct APIntCodec(pub APInt);

impl Serialize for APIntCodec {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (val, bit): (i128, u8) = (self.0.as_signed(), self.0.bits());
        let s = format_smolstr!("i{}:{}", bit, val);
        serializer.serialize_str(&s)
    }
}
impl<'de> Deserialize<'de> for APIntCodec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: &str = Deserialize::deserialize(deserializer)?;
        let errmsg = || serde::de::Error::custom(format!("invalid APInt '{s}'"));

        let Some(rest) = s.strip_prefix('i') else {
            return Err(errmsg());
        };
        let mut parts = rest.split(':');
        let bits_str = parts.next().ok_or_else(errmsg)?;
        let val_str = parts.next().ok_or_else(errmsg)?;
        if parts.next().is_some() {
            return Err(errmsg());
        }

        let bits: u8 = bits_str.parse().map_err(|_| errmsg())?;
        if bits > 128 {
            return Err(errmsg());
        }
        let val: i128 = val_str.parse().map_err(|_| errmsg())?;
        Ok(APIntCodec(APInt::new(val as u128, bits)))
    }
}
