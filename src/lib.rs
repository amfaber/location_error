use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use std::{borrow::Cow, panic::Location as StdLocation};

use anyhow::anyhow;

pub type LocationResult<T> = Result<T, LocationError>;

#[derive(Debug, Deserialize, Serialize)]
pub struct LocationError {
    #[serde(
        deserialize_with = "deserialize_source",
        serialize_with = "serialize_source"
    )]
    pub source: anyhow::Error,
    pub backtrace: Vec<Location>,
}

fn serialize_source<S: Serializer>(
    value: &anyhow::Error,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(&format!("{value:#?}"))
}

fn deserialize_source<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<anyhow::Error, D::Error> {
    struct SourceVisitor;
    impl Visitor<'_> for SourceVisitor {
        type Value = anyhow::Error;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("error string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let err_str = DisplayString(v.into());
            Ok(anyhow!(err_str))
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            let err_str = DisplayString(v);
            Ok(anyhow!(err_str))
        }
    }
    deserializer.deserialize_str(SourceVisitor)
}

#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Location {
    pub file: Cow<'static, str>,
    pub line: u32,
    pub col: u32,
}

impl From<&'static StdLocation<'static>> for Location {
    fn from(value: &'static StdLocation) -> Self {
        Self {
            file: value.file().into(),
            line: value.line(),
            col: value.column(),
        }
    }
}

impl std::fmt::Debug for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}:{}:{}", self.file, self.line, self.col,))
    }
}

impl std::fmt::Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, f)
    }
}

impl std::fmt::Display for LocationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Self as std::fmt::Debug>::fmt(self, f)
    }
}

impl LocationError {
    #[track_caller]
    pub fn add_location(mut self) -> Self {
        let caller = std::panic::Location::caller();
        self.backtrace.push(caller.into());
        self
    }

    pub fn context<C>(mut self, context: C) -> Self
    where
        C: std::fmt::Display + Send + Sync + 'static,
    {
        self.source = self.source.context(context);

        self
    }
}

pub trait AddLocation<T> {
    fn loc(self) -> LocationResult<T>;
    fn context<C>(self, context: C) -> LocationResult<T>
    where
        C: std::fmt::Display + Send + Sync + 'static;
}

impl<T> AddLocation<T> for Result<T, LocationError> {
    #[track_caller]
    fn loc(self) -> LocationResult<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => Err(err.add_location()),
        }
    }

    fn context<C>(self, context: C) -> Self
    where
        C: std::fmt::Display + Send + Sync + 'static,
    {
        match self {
            Ok(_) => self,
            Err(err) => Err(err.context(context)),
        }
    }
}

impl LocationError {
    #[track_caller]
    pub fn new<E>(value: E) -> Self
    where
        anyhow::Error: From<E>,
    {
        let caller = std::panic::Location::caller().into();
        let backtrace = vec![caller];
        let source = anyhow::Error::from(value);
        LocationError { source, backtrace }
    }
}

pub trait ToLocation<T> {
    fn loc(self) -> LocationResult<T>;
    fn no_loc(self) -> LocationResult<T>;
}

impl<T, E> ToLocation<T> for Result<T, E>
where
    anyhow::Error: From<E>,
    E: std::fmt::Debug,
{
    #[track_caller]
    fn loc(self) -> LocationResult<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => Err(LocationError::new(err)),
        }
    }

    fn no_loc(self) -> LocationResult<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(err) => Err(LocationError {
                source: anyhow::Error::from(err),
                backtrace: vec![],
            }),
        }
    }
}

const OPTION_ERR: &str = "Option was None";
impl<T> ToLocation<T> for Option<T> {
    #[track_caller]
    fn loc(self) -> LocationResult<T> {
        match self {
            Some(some) => Ok(some),
            None => Err(LocationError::new(anyhow!(OPTION_ERR))),
        }
    }

    fn no_loc(self) -> LocationResult<T> {
        match self {
            Some(some) => Ok(some),
            None => Err(LocationError {
                source: anyhow!(OPTION_ERR),
                backtrace: vec![],
            }),
        }
    }
}

#[test]
fn location_error_serde() {
    let err = Err::<(), _>(anyhow!("Some message")).loc().unwrap_err();

    let json = dbg!(serde_json::to_string_pretty(&err).unwrap());

    let recovered_err = serde_json::from_str::<LocationError>(&json).unwrap();

    dbg!(recovered_err);
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct DisplayString(pub String);

impl std::error::Error for DisplayString {}

impl std::fmt::Debug for DisplayString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for DisplayString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for DisplayString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<DisplayString> for String {
    fn from(value: DisplayString) -> Self {
        value.0
    }
}
