//! Domain wrapper types for the repository listing BDD tests.

use std::fmt;
use std::str::FromStr;

/// Page number for pagination (1-based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageNumber(u32);

impl PageNumber {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for PageNumber {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.parse::<u32>().map_err(|error| error.to_string())?;
        if value == 0 {
            return Err("PageNumber must be >= 1".to_owned());
        }

        Ok(Self(value))
    }
}

impl fmt::Display for PageNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Count of pull requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PullRequestCount(u32);

impl PullRequestCount {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for PullRequestCount {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>().map(Self)
    }
}

impl fmt::Display for PullRequestCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Total number of pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PageCount(u32);

impl PageCount {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for PageCount {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.parse::<u32>().map_err(|error| error.to_string())?;
        if value == 0 {
            return Err("PageCount must be >= 1".to_owned());
        }

        Ok(Self::new(value))
    }
}

impl fmt::Display for PageCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Rate limit remaining count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RateLimitCount(u32);

impl RateLimitCount {
    pub(crate) const fn new(value: u32) -> Self {
        Self(value)
    }

    pub(crate) const fn value(self) -> u32 {
        self.0
    }
}

impl FromStr for RateLimitCount {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u32>().map(Self::new)
    }
}

impl fmt::Display for RateLimitCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
