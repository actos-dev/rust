//! Cursor-based pagination and stream generation.

use std::future::Future;
use std::vec::IntoIter;

use crate::error::{Error, Result};

/// A single page of items returned by a cursor-paginated endpoint.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Page<T> {
    /// Items contained in the current page.
    pub items: Vec<T>,
    /// Opaque cursor pointing to the next page, or `None` if this is the final page.
    pub next_cursor: Option<String>,
}

impl<T> Page<T> {
    /// Creates a new page of items with an optional next cursor.
    #[must_use]
    pub fn new(items: Vec<T>, next_cursor: Option<String>) -> Self {
        Self { items, next_cursor }
    }

    /// Returns `true` if another page is available.
    #[must_use]
    pub fn has_next(&self) -> bool {
        self.next_cursor.is_some()
    }
}

enum StreamState<T, F> {
    Yielding {
        items: IntoIter<T>,
        next_cursor: Option<String>,
        fetcher: F,
    },
    Fetching {
        cursor: Option<String>,
        fetcher: F,
    },
    Done,
}

/// Creates an asynchronous [`futures_core::Stream`] that transparently iterates through
/// all items across pages by querying `fetcher` with subsequent cursor tokens.
pub fn paginate_stream<T, F, Fut>(
    fetcher: F,
) -> impl futures_core::Stream<Item = Result<T, Error>> + Send
where
    T: Send + 'static,
    F: FnMut(Option<String>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Page<T>, Error>> + Send + 'static,
{
    paginate_stream_with_cursor(None, fetcher)
}

/// Creates an asynchronous [`futures_core::Stream`] with an initial resume cursor.
pub fn paginate_stream_with_cursor<T, F, Fut>(
    initial_cursor: Option<String>,
    fetcher: F,
) -> impl futures_core::Stream<Item = Result<T, Error>> + Send
where
    T: Send + 'static,
    F: FnMut(Option<String>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Page<T>, Error>> + Send + 'static,
{
    futures_util::stream::unfold(
        StreamState::Fetching {
            cursor: initial_cursor,
            fetcher,
        },
        |mut state| async move {
            loop {
                match state {
                    StreamState::Yielding {
                        mut items,
                        next_cursor,
                        fetcher,
                    } => {
                        if let Some(item) = items.next() {
                            let next_state = StreamState::Yielding {
                                items,
                                next_cursor,
                                fetcher,
                            };
                            return Some((Ok(item), next_state));
                        }
                        match next_cursor {
                            Some(cursor) => {
                                state = StreamState::Fetching {
                                    cursor: Some(cursor),
                                    fetcher,
                                };
                            }
                            None => return None,
                        }
                    }
                    StreamState::Fetching {
                        cursor,
                        mut fetcher,
                    } => match fetcher(cursor).await {
                        Ok(page) => {
                            let mut items = page.items.into_iter();
                            if let Some(item) = items.next() {
                                let next_state = StreamState::Yielding {
                                    items,
                                    next_cursor: page.next_cursor,
                                    fetcher,
                                };
                                return Some((Ok(item), next_state));
                            }
                            match page.next_cursor {
                                Some(next_cursor) => {
                                    state = StreamState::Fetching {
                                        cursor: Some(next_cursor),
                                        fetcher,
                                    };
                                }
                                None => return None,
                            }
                        }
                        Err(e) => return Some((Err(e), StreamState::Done)),
                    },
                    StreamState::Done => return None,
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::StreamExt;

    #[tokio::test]
    async fn test_page_constructors() {
        let page = Page::new(vec![1, 2, 3], Some("cursor_abc".to_string()));
        assert_eq!(page.items, vec![1, 2, 3]);
        assert_eq!(page.next_cursor.as_deref(), Some("cursor_abc"));
        assert!(page.has_next());

        let last_page = Page::new(vec![4, 5], None);
        assert!(!last_page.has_next());
    }

    #[tokio::test]
    async fn test_paginate_stream_multi_page() {
        let fetcher = |cursor: Option<String>| async move {
            match cursor.as_deref() {
                None => Ok(Page::new(vec![1, 2], Some("c1".to_string()))),
                Some("c1") => Ok(Page::new(vec![3, 4], Some("c2".to_string()))),
                Some("c2") => Ok(Page::new(vec![5], None)),
                Some(_) => Err(Error::Config("unexpected cursor".to_string())),
            }
        };

        let stream = paginate_stream(fetcher);
        let items: Vec<Result<i32, Error>> = stream.collect().await;

        assert_eq!(items.len(), 5);
        let values: Vec<i32> = items.into_iter().map(|r| r.expect("ok item")).collect();
        assert_eq!(values, vec![1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn test_paginate_stream_with_initial_cursor() {
        let fetcher = |cursor: Option<String>| async move {
            match cursor.as_deref() {
                Some("resume_at") => Ok(Page::new(vec![10, 20], None)),
                _ => Err(Error::Config("expected resume_at".to_string())),
            }
        };

        let stream = paginate_stream_with_cursor(Some("resume_at".to_string()), fetcher);
        let items: Vec<Result<i32, Error>> = stream.collect().await;

        assert_eq!(items.len(), 2);
        let values: Vec<i32> = items.into_iter().map(|r| r.expect("ok item")).collect();
        assert_eq!(values, vec![10, 20]);
    }

    #[tokio::test]
    async fn test_paginate_stream_error_halts_stream() {
        let fetcher = |cursor: Option<String>| async move {
            match cursor.as_deref() {
                None => Ok(Page::new(vec![1, 2], Some("bad_cursor".to_string()))),
                Some("bad_cursor") => Err(Error::Config("server error".to_string())),
                _ => Ok(Page::new(vec![], None)),
            }
        };

        let stream = paginate_stream(fetcher);
        let items: Vec<Result<i32, Error>> = stream.collect().await;

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].as_ref().unwrap(), &1);
        assert_eq!(items[1].as_ref().unwrap(), &2);
        assert!(items[2].is_err());
    }

    #[tokio::test]
    async fn test_paginate_stream_empty_page_with_next_cursor() {
        let fetcher = |cursor: Option<String>| async move {
            match cursor.as_deref() {
                None => Ok(Page::new(vec![], Some("c1".to_string()))),
                Some("c1") => Ok(Page::new(vec![42], None)),
                _ => Ok(Page::new(vec![], None)),
            }
        };

        let stream = paginate_stream(fetcher);
        let items: Vec<Result<i32, Error>> = stream.collect().await;

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].as_ref().unwrap(), &42);
    }
}
