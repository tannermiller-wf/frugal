// Autogenerated by Frugal Compiler (2.22.0)
// DO NOT EDIT UNLESS YOU ARE SURE THAT YOU KNOW WHAT YOU ARE DOING

pub const REDEF_CONST: i32 = rust::CONST_I32_FROM_BASE;

lazy_static! {
    pub static ref CONST_THING: rust::Thing = rust::Thing {
        an_id: Some(1),
        a_string: Some("some string".into()),
    };
}

pub const DEFAULT_ID: Id = -1;

pub const OTHER_DEFAULT: Id = DEFAULT_ID;

pub const THIRTYFOUR: i8 = 34;

lazy_static! {
    pub static ref MAPCONSTANT: BTreeMap<String, String> = {
        let mut m = BTreeMap::new();
        m.insert("hello".into(), "world".into());
        m.insert("goodnight".into(), "moon".into());
        m
    };
}

lazy_static! {
    pub static ref CONSTEVENT1: Event = Event {
        id: Some(-2),
        message: Some("first one".into()),
    };
}

lazy_static! {
    pub static ref CONSTEVENT2: Event = Event {
        id: Some(-7),
        message: Some("second one".into()),
    };
}

lazy_static! {
    pub static ref NUMSLIST: Vec<i32> = vec![2, 4, 7, 1];
}

lazy_static! {
    pub static ref NUMSSET: BTreeSet<Int> = {
        let mut s = BTreeSet::new();
        s.insert(1);
        s.insert(3);
        s.insert(8);
        s.insert(0);
        s
    };
}

lazy_static! {
    pub static ref MAPCONSTANT2: BTreeMap<String, Event> = {
        let mut m = BTreeMap::new();
        m.insert(
            "hello".into(),
            Event {
                id: Some(-2),
                message: Some("first here".into()),
            },
        );
        m
    };
}

lazy_static! {
    pub static ref BIN_CONST: Vec<u8> = b"hello".to_vec();
}

pub const TRUE_CONSTANT: bool = true;

pub const FALSE_CONSTANT: bool = false;

lazy_static! {
    pub static ref CONST_HC: HealthCondition = HealthCondition::Warn;
}

lazy_static! {
    pub static ref EVIL_STRING: String = "thin'g\" \"".into();
}

lazy_static! {
    pub static ref EVIL_STRING2: String = "th'ing\"ad\"f".into();
}

lazy_static! {
    pub static ref CONST_LOWER: TestLowercase = TestLowercase {
        lowercase_int: Some(2),
    };
}

pub type Id = i64;

pub type Int = i32;

pub type Request = BTreeMap<Int, String>;

pub type T1String = String;

pub type T2String = T1String;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum HealthCondition {
    Pass = 1,
    Warn = 2,
    Fail = 3,
    Unknown = 4,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ItsAnEnum {
    First = 2,
    Second = 3,
    Third = 4,
    Fourth = 5,
    Fifth = 6,
    Sixith = 7,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TestBase {
    pub base_struct: Option<rust::Thing>,
}

impl TestBase {
    pub fn new<F0>(base_struct: F0) -> TestBase
    where
        F0: Into<Option<rust::Thing>>,
    {
        TestBase {
            base_struct: base_struct.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TestLowercase {
    pub lowercase_int: Option<i32>,
}

impl TestLowercase {
    pub fn new<F0>(lowercase_int: F0) -> TestLowercase
    where
        F0: Into<Option<i32>>,
    {
        TestLowercase {
            lowercase_int: lowercase_int.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Event {
    pub id: Option<Id>,
    pub message: Option<String>,
}

impl Event {
    pub fn new<F0, F1>(id: F0, message: F1) -> Event
    where
        F0: Into<Option<Id>>,
        F1: Into<Option<String>>,
    {
        Event {
            id: id.into().or(Some(DEFAULT_ID)),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TestingDefaults {
    pub id2: Option<Id>,
    pub ev1: Option<Event>,
    pub ev2: Option<Event>,
    pub id: Option<Id>,
    pub thing: Option<String>,
    pub thing2: Option<String>,
    pub listfield: Option<Vec<Int>>,
    pub id3: Option<Id>,
    pub bin_field: Option<Vec<u8>>,
    pub bin_field2: Option<Vec<u8>>,
    pub bin_field3: Option<Vec<u8>>,
    pub bin_field4: Option<Vec<u8>>,
    pub list2: Option<Vec<Int>>,
    pub list3: Option<Vec<Int>>,
    pub list4: Option<Vec<Int>>,
    pub a_map: Option<BTreeMap<String, String>>,
    pub status: HealthCondition,
    pub base_status: rust::BaseHealthCondition,
}

impl TestingDefaults {
    pub fn new<F0, F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17>(
        id2: F0,
        ev1: F1,
        ev2: F2,
        id: F3,
        thing: F4,
        thing2: F5,
        listfield: F6,
        id3: F7,
        bin_field: F8,
        bin_field2: F9,
        bin_field3: F10,
        bin_field4: F11,
        list2: F12,
        list3: F13,
        list4: F14,
        a_map: F15,
        status: F16,
        base_status: F17,
    ) -> TestingDefaults
    where
        F0: Into<Option<Id>>,
        F1: Into<Option<Event>>,
        F2: Into<Option<Event>>,
        F3: Into<Option<Id>>,
        F4: Into<Option<String>>,
        F5: Into<Option<String>>,
        F6: Into<Option<Vec<Int>>>,
        F7: Into<Option<Id>>,
        F8: Into<Option<Vec<u8>>>,
        F9: Into<Option<Vec<u8>>>,
        F10: Into<Option<Vec<u8>>>,
        F11: Into<Option<Vec<u8>>>,
        F12: Into<Option<Vec<Int>>>,
        F13: Into<Option<Vec<Int>>>,
        F14: Into<Option<Vec<Int>>>,
        F15: Into<Option<BTreeMap<String, String>>>,
        F16: Into<Option<HealthCondition>>,
        F17: Into<Option<rust::BaseHealthCondition>>,
    {
        TestingDefaults {
            id2: id2.into().or(Some(DEFAULT_ID)),
            ev1: ev1.into().or(Some(Event {
                id: Some(DEFAULT_ID),
                message: Some("a message".into()),
            })),
            ev2: ev2.into().or(Some(Event {
                id: Some(5),
                message: Some("a message2".into()),
            })),
            id: id.into().or(Some(-2)),
            thing: thing.into().or(Some("a constant".into())),
            thing2: thing2.into().or(Some("another constant".into())),
            listfield: listfield.into().or(Some(vec![1, 2, 3, 4, 5])),
            id3: id3.into().or(Some(OTHER_DEFAULT)),
            bin_field: bin_field.into(),
            bin_field2: bin_field2.into(),
            bin_field3: bin_field3.into(),
            bin_field4: bin_field4.into().or(Some(BIN_CONST)),
            list2: list2.into().or(Some(vec![1, 3, 4, 5, 8])),
            list3: list3.into(),
            list4: list4.into().or(Some(vec![1, 2, 3, 6])),
            a_map: a_map.into().or(Some({
                let mut m = BTreeMap::new();
                m.insert("k1".into(), "v1".into());
                m.insert("k2".into(), "v2".into());
                m
            })),
            status: status.into().unwrap_or(HealthCondition::Pass),
            base_status: base_status
                .into()
                .unwrap_or(rust::BaseHealthCondition::Fail),
        }
    }
}
