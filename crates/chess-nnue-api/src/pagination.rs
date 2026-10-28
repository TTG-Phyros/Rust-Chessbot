//! Cursor pagination: lists are newest first; `nextCursor` (absent on the
//! last page) is passed back as `cursor` for the next page.

use std::str::FromStr;

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::problem::{ApiProblem, ApiResult};

/// Items per page unless the request says otherwise.
const DEFAULT_LIMIT: usize = 50;

/// Paging parameters of a list.
#[derive(Debug, Clone, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PageQuery {
    /// Items per page, 1 to 200 (default 50).
    #[param(minimum = 1, maximum = 200)]
    pub(crate) limit: Option<usize>,
    /// `nextCursor` of the previous page.
    pub(crate) cursor: Option<String>,
}

impl PageQuery {
    /// The limit, checked.
    pub(crate) fn limit(&self) -> ApiResult<usize> {
        match self.limit {
            None => Ok(DEFAULT_LIMIT),
            Some(limit) if (1..=chess_jobs::MAX_PAGE).contains(&limit) => Ok(limit),
            Some(_) => Err(ApiProblem::bad_request(format!(
                "limit must be between 1 and {}",
                chess_jobs::MAX_PAGE
            ))),
        }
    }

    /// The id to continue after, from the cursor.
    pub(crate) fn after<T: FromStr>(&self) -> ApiResult<Option<T>> {
        let Some(cursor) = &self.cursor else {
            return Ok(None);
        };
        let invalid = || ApiProblem::bad_request("invalid cursor");
        let bytes: Vec<u8> = (0..cursor.len())
            .step_by(2)
            .map(|i| {
                cursor
                    .get(i..i + 2)
                    .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            })
            .collect::<Option<_>>()
            .ok_or_else(invalid)?;
        let text = String::from_utf8(bytes).map_err(|_| invalid())?;
        let id = text.strip_prefix("v1:").ok_or_else(invalid)?;
        id.parse().map(Some).map_err(|_| invalid())
    }
}

/// The cursor of the page after the one ending with `last`, if the page is full.
pub(crate) fn next_cursor(len: usize, limit: usize, last: Option<impl ToString>) -> Option<String> {
    (len == limit).then_some(())?;
    let text = format!("v1:{}", last?.to_string());
    Some(text.bytes().map(|b| format!("{b:02x}")).collect())
}

macro_rules! page {
    ($name:ident, $item:ty, $what:literal) => {
        #[doc = concat!("A page of ", $what, ", newest first.")]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
        #[serde(rename_all = "camelCase")]
        pub(crate) struct $name {
            /// Items of the page.
            pub(crate) items: Vec<$item>,
            /// Cursor of the next page; absent on the last one.
            #[serde(default, skip_serializing_if = "Option::is_none")]
            pub(crate) next_cursor: Option<String>,
        }
    };
}

page!(
    ArchitecturePage,
    chess_jobs::StoredArchitecture,
    "saved architectures"
);
page!(DatasetPage, chess_jobs::Dataset, "datasets");
page!(JobPage, chess_jobs::Job, "runs");
page!(ModelPage, crate::routes::models::ModelView, "models");

#[cfg(test)]
mod tests {
    use chess_jobs::JobId;

    use super::*;

    #[test]
    fn cursors_round_trip_and_reject_garbage() {
        let id = JobId::generate();
        let cursor = next_cursor(2, 2, Some(id)).unwrap();
        let query = PageQuery {
            limit: Some(2),
            cursor: Some(cursor),
        };
        assert_eq!(query.after::<JobId>().unwrap(), Some(id));
        assert_eq!(next_cursor(1, 2, Some(id)), None);
        for bad in ["zz", "abc", "763a", &"x".repeat(10)] {
            let query = PageQuery {
                limit: None,
                cursor: Some(bad.into()),
            };
            assert!(query.after::<JobId>().is_err(), "{bad}");
        }
        assert!(
            PageQuery {
                limit: Some(0),
                cursor: None
            }
            .limit()
            .is_err()
        );
    }
}
