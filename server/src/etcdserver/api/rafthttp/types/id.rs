use std::fmt;
use std::str::FromStr;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ID(u64);

impl fmt::LowerHex for ID {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

impl ID {
    pub fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl FromStr for ID {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let i = u64::from_str_radix(s, 16)?;
        Ok(ID(i))
    }
}

struct IDSlice(Vec<ID>);

impl IDSlice {
    fn sort(&mut self) {
        self.0.sort();
    }
}

impl std::ops::Deref for IDSlice {
    type Target = [ID];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for IDSlice {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}