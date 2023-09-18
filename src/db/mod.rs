mod users;
use azure_data_tables::prelude::TableServiceClient;
use futures::StreamExt;
use rand::Rng;
pub use users::*;
mod assets;
pub use assets::*;
mod runs;
pub use runs::*;

use crate::util::Result;
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

pub async fn get_entities<T, U>(
    client: &TableServiceClient,
    table_name: &str,
    page: usize,
) -> crate::util::Result<Option<Vec<U>>>
where
    T: for<'a> Deserialize<'a> + Send + Sync,
    U: From<T>,
{
    // Skip to the desired page in the stream
    let page = client
        .table_client(table_name)
        .query()
        .into_stream::<T>()
        .skip(page - 1)
        .next()
        .await;

    // Map the page results to the output type
    Ok(page.transpose()?.map(|response| {
        response
            .entities
            .into_iter()
            .map(|e| e.into())
            .collect::<Vec<_>>()
    }))
}
