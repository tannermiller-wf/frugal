// Autogenerated by Frugal Compiler (2.23.0)
// DO NOT EDIT UNLESS YOU ARE SURE THAT YOU KNOW WHAT YOU ARE DOING

#![allow(deprecated)]
#![allow(unused_imports)]

extern crate frugal;
extern crate thrift;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate futures;
extern crate tower_service;
extern crate tower_web;

use std::collections::{BTreeMap, BTreeSet};

pub const CONST_I32_FROM_BASE: i32 = 582;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BaseHealthCondition {
    Pass = 1,
    Warn = 2,
    Fail = 3,
    Unknown = 4,
}

impl BaseHealthCondition {
    pub fn from_i32(i: i32) -> thrift::Result<BaseHealthCondition> {
        match i {
            i if BaseHealthCondition::Pass as i32 == i => Ok(BaseHealthCondition::Pass),
            i if BaseHealthCondition::Warn as i32 == i => Ok(BaseHealthCondition::Warn),
            i if BaseHealthCondition::Fail as i32 == i => Ok(BaseHealthCondition::Fail),
            i if BaseHealthCondition::Unknown as i32 == i => Ok(BaseHealthCondition::Unknown),
            _ => Err(thrift::new_protocol_error(
                thrift::ProtocolErrorKind::InvalidData,
                format!("{} is not a valid integer value for BaseHealthCondition", i),
            )),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Thing {
    pub an_id: Option<i32>,
    pub a_string: Option<String>,
}

impl Thing {
    pub fn read<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        iprot.read_struct_begin()?;
        loop {
            let field_id = iprot.read_field_begin()?;
            if field_id.field_type == thrift::protocol::TType::Stop {
                break;
            };
            match field_id.id {
                Some(1) => self.read_field_1(iprot)?,
                Some(2) => self.read_field_2(iprot)?,
                _ => iprot.skip(field_id.field_type)?,
            };
            iprot.read_field_end()?;
        }
        iprot.read_struct_end()
    }

    fn read_field_1<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        let an_id = iprot.read_i32()?;
        self.an_id = Some(an_id);
        Ok(())
    }

    fn read_field_2<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        let a_string = iprot.read_string()?;
        self.a_string = Some(a_string);
        Ok(())
    }

    pub fn write<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        oprot.write_struct_begin(&thrift::protocol::TStructIdentifier::new("thing"))?;
        self.write_field_1(oprot)?;
        self.write_field_2(oprot)?;
        oprot.write_field_stop()?;
        oprot.write_struct_end()
    }

    fn write_field_1<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        let an_id = match self.an_id {
            Some(an_id) => an_id,
            None => return Ok(()),
        };
        oprot.write_field_begin(&thrift::protocol::TFieldIdentifier {
            name: Some("an_id".into()),
            field_type: thrift::protocol::TType::I32,
            id: Some(1),
        })?;
        oprot.write_i32(an_id)?;
        oprot.write_field_end()
    }

    fn write_field_2<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        let a_string = match self.a_string {
            Some(ref a_string) => a_string,
            None => return Ok(()),
        };
        oprot.write_field_begin(&thrift::protocol::TFieldIdentifier {
            name: Some("a_string".into()),
            field_type: thrift::protocol::TType::String,
            id: Some(2),
        })?;
        oprot.write_string(a_string)?;
        oprot.write_field_end()
    }
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct NestedThing {
    pub things: Option<Vec<Thing>>,
}

impl NestedThing {
    pub fn read<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        iprot.read_struct_begin()?;
        loop {
            let field_id = iprot.read_field_begin()?;
            if field_id.field_type == thrift::protocol::TType::Stop {
                break;
            };
            match field_id.id {
                Some(1) => self.read_field_1(iprot)?,
                _ => iprot.skip(field_id.field_type)?,
            };
            iprot.read_field_end()?;
        }
        iprot.read_struct_end()
    }

    fn read_field_1<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        let list_id = iprot.read_list_begin()?;
        let mut things = Vec::with_capacity(list_id.size as usize);
        for _ in 0..list_id.size {
            let mut things_item = Thing::default();
            things_item.read(iprot)?;
            things.push(things_item);
        }
        iprot.read_list_end()?;
        self.things = Some(things);
        Ok(())
    }

    pub fn write<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        oprot.write_struct_begin(&thrift::protocol::TStructIdentifier::new("nested_thing"))?;
        self.write_field_1(oprot)?;
        oprot.write_field_stop()?;
        oprot.write_struct_end()
    }

    fn write_field_1<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        let things = match self.things {
            Some(ref things) => things,
            None => return Ok(()),
        };
        oprot.write_field_begin(&thrift::protocol::TFieldIdentifier {
            name: Some("things".into()),
            field_type: thrift::protocol::TType::List,
            id: Some(1),
        })?;
        oprot.write_list_begin(&thrift::protocol::TListIdentifier {
            element_type: thrift::protocol::TType::Struct,
            size: things.len() as i32,
        })?;
        for things_item in things {
            things_item.write(oprot)?;
        }
        oprot.write_list_end()?;
        oprot.write_field_end()
    }
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct ApiException {}

impl ApiException {
    pub fn read<R, T>(&mut self, iprot: &mut T) -> thrift::Result<()>
    where
        R: thrift::transport::TReadTransport,
        T: thrift::protocol::TInputProtocol<R>,
    {
        iprot.read_struct_begin()?;
        loop {
            let field_id = iprot.read_field_begin()?;
            if field_id.field_type == thrift::protocol::TType::Stop {
                break;
            };
            match field_id.id {
                _ => iprot.skip(field_id.field_type)?,
            };
            iprot.read_field_end()?;
        }
        iprot.read_struct_end()
    }

    pub fn write<W, T>(&self, oprot: &mut T) -> thrift::Result<()>
    where
        W: thrift::transport::TWriteTransport,
        T: thrift::protocol::TOutputProtocol<W>,
    {
        oprot.write_struct_begin(&thrift::protocol::TStructIdentifier::new("api_exception"))?;
        oprot.write_field_stop()?;
        oprot.write_struct_end()
    }
}

impl std::error::Error for ApiException {}

impl std::fmt::Display for ApiException {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{:?}", self)
    }
}

pub mod basefoo_service;
