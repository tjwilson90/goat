use std::fmt;
use std::fmt::{Debug, Display};

use crate::RandId;
use serde::{Deserialize, Serialize};

macro_rules! declare_id {
    ($name:ident, $inner:ty) => {
        #[repr(transparent)]
        #[derive(Clone, Copy, Deserialize, Eq, Hash, PartialEq, Serialize)]
        pub struct $name(pub $inner);

        impl From<$inner> for $name {
            fn from(inner: $inner) -> Self {
                Self(inner)
            }
        }

        impl Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                Debug::fmt(&self.0, f)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                Display::fmt(&self.0, f)
            }
        }
    };
}

declare_id!(GameId, RandId);
declare_id!(UserId, RandId);
declare_id!(PlayerIdx, u8);

impl PlayerIdx {
    pub fn idx(self) -> usize {
        self.0 as usize
    }
}
//
// impl FromStr for GameId {
//     type Err = <u64 as FromStr>::Err;
//
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Ok(Self(s.parse()?))
//     }
// }
//
// impl FromStr for UserId {
//     type Err = <u64 as FromStr>::Err;
//
//     fn from_str(s: &str) -> Result<Self, Self::Err> {
//         Ok(Self(s.parse()?))
//     }
// }
