pub mod users;
pub use users::*;

use serde::{Deserialize, Serialize};

use std::num::NonZeroU8;

use time::{
    format_description::well_known::{
        iso8601::{self, TimePrecision},
        Iso8601,
    },
    OffsetDateTime,
};
const FORMAT: Iso8601<6651332276410551414894041209048662016> = Iso8601::<
    {
        iso8601::Config::DEFAULT
            .set_year_is_six_digits(false)
            .set_time_precision(TimePrecision::Second {
                decimal_digits: NonZeroU8::new(7),
            })
            .encode()
    },
>;
time::serde::format_description!(blueprint_datetime, OffsetDateTime, FORMAT);

macro_rules! edm_tag_serializer {
    ($id:ident => $ty:literal) => {
        fn $id<S>(_: &(), ser: S) -> ::std::result::Result<S::Ok, S::Error>
        where
            S: ::serde::Serializer,
        {
            ser.serialize_str($ty)
        }
    };
}

edm_tag_serializer!(edm_datetime => "Edm.DateTime");

/// A wrapper around [`OffsetDateTime`] which provides a more compatible (de)serialization method that meshes well with Azure table storage.
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(transparent)]
#[repr(transparent)]
pub struct DateTime(#[serde(with = "blueprint_datetime")] pub OffsetDateTime);

impl From<OffsetDateTime> for DateTime {
    fn from(value: OffsetDateTime) -> Self {
        DateTime(value)
    }
}

impl From<DateTime> for OffsetDateTime {
    fn from(value: DateTime) -> Self {
        value.0
    }
}
