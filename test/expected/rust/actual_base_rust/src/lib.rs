// Autogenerated by Frugal Compiler (2.22.0)
// DO NOT EDIT UNLESS YOU ARE SURE THAT YOU KNOW WHAT YOU ARE DOING

pub const CONST_I32_FROM_BASE: i32 = 582;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BaseHealthCondition {
    Pass = 1,
    Warn = 2,
    Fail = 3,
    Unknown = 4,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Thing {
    pub an_id: Option<i32>,
    pub a_string: Option<String>,
}

impl Thing {
    pub fn new<F0, F1>(an_id: F0, a_string: F1) -> Thing
    where
        F0: Into<Option<i32>>,
        F1: Into<Option<String>>,
    {
        Thing {
            an_id: an_id.into(),
            a_string: a_string.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct NestedThing {
    pub things: Option<Vec<Thing>>,
}

impl NestedThing {
    pub fn new<F0>(things: F0) -> NestedThing
    where
        F0: Into<Option<Vec<Thing>>>,
    {
        NestedThing {
            things: things.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ApiException {}

impl ApiException {
    pub fn new() -> ApiException {
        ApiException {}
    }
}

pub mod basefoo_service;