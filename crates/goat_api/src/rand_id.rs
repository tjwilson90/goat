use rand::distributions::{Distribution, Standard};
use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::fmt::Display;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RandId(u128);

impl RandId {
    pub fn from_hash(hash: &[u8]) -> Self {
        let hash = hash[..16].try_into().unwrap();
        Self(u128::from_le_bytes(hash) & 0xffff_ffff_ffff_ffff_ffff_ffff)
    }

    fn to_bytes(self) -> [u8; 16] {
        let mut buf = [0; 16];
        let mut val = self.0;
        for b in &mut buf {
            *b = encode((val & 0x3f) as u8);
            val >>= 6;
        }
        buf
    }
}

impl Distribution<RandId> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> RandId {
        RandId(rng.gen::<u128>() & 0xffff_ffff_ffff_ffff_ffff_ffff)
    }
}

impl Display for RandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let buf = self.to_bytes();
        f.write_str(std::str::from_utf8(&buf).unwrap())
    }
}

impl Serialize for RandId {
    fn serialize<S>(&self, ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let buf = self.to_bytes();
        ser.serialize_str(std::str::from_utf8(&buf).unwrap())
    }
}

impl<'de> Deserialize<'de> for RandId {
    fn deserialize<D>(des: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = RandId;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.write_str("an id")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let mut val = 0;
                v.bytes().for_each(|b| {
                    val <<= 6;
                    val += decode(b) as u128;
                });
                Ok(RandId(val))
            }
        }
        des.deserialize_str(Visitor)
    }
}

fn encode(b: u8) -> u8 {
    match b {
        0..=11 => b + 46,
        12..=37 => b + 53,
        _ => b + 59,
    }
}

fn decode(b: u8) -> u8 {
    match b {
        46..=57 => b - 46,
        65..=90 => b - 53,
        _ => b - 59,
    }
}
