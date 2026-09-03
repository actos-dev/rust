//! File upload endpoints and multipart attachment streaming.

use std::path::{Path, PathBuf};

pub use actos_types::upload::UploadResponse;
use reqwest::Method;
use tokio::io::AsyncRead;

use crate::error::{Error, Result};
use crate::transport::Transport;

/// Data source for uploading a file to Actos.
pub enum UploadSource {
    /// Upload from a file path on the local filesystem. Streams asynchronously from disk.
    Path(PathBuf),
    /// In-memory byte buffer.
    Bytes(Vec<u8>),
    /// Arbitrary asynchronous stream implementing [`AsyncRead`].
    Stream {
        /// Reader stream boxed with thread-safety bounds.
        reader: Box<dyn AsyncRead + Send + Sync + Unpin>,
        /// Optional known length in bytes (allows setting Content-Length on the multipart chunk).
        length: Option<u64>,
    },
}

impl std::fmt::Debug for UploadSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Path(p) => f.debug_tuple("Path").field(p).finish(),
            Self::Bytes(b) => f
                .debug_tuple("Bytes")
                .field(&format_args!("{} bytes", b.len()))
                .finish(),
            Self::Stream { length, .. } => f
                .debug_struct("Stream")
                .field("length", length)
                .finish_non_exhaustive(),
        }
    }
}

impl From<PathBuf> for UploadSource {
    fn from(p: PathBuf) -> Self {
        Self::Path(p)
    }
}

impl From<&Path> for UploadSource {
    fn from(p: &Path) -> Self {
        Self::Path(p.to_path_buf())
    }
}

impl From<&PathBuf> for UploadSource {
    fn from(p: &PathBuf) -> Self {
        Self::Path(p.clone())
    }
}

impl From<&str> for UploadSource {
    fn from(s: &str) -> Self {
        Self::Path(PathBuf::from(s))
    }
}

impl From<String> for UploadSource {
    fn from(s: String) -> Self {
        Self::Path(PathBuf::from(s))
    }
}

impl From<Vec<u8>> for UploadSource {
    fn from(bytes: Vec<u8>) -> Self {
        Self::Bytes(bytes)
    }
}

impl From<&[u8]> for UploadSource {
    fn from(slice: &[u8]) -> Self {
        Self::Bytes(slice.to_vec())
    }
}

impl<const N: usize> From<&[u8; N]> for UploadSource {
    fn from(slice: &[u8; N]) -> Self {
        Self::Bytes(slice.to_vec())
    }
}

impl UploadSource {
    /// Creates an upload source from a filesystem path.
    pub fn from_path(path: impl AsRef<Path>) -> Self {
        Self::Path(path.as_ref().to_path_buf())
    }

    /// Creates an upload source from a byte buffer.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::Bytes(bytes.into())
    }

    /// Creates an upload source from an [`AsyncRead`] stream without loading the full file into memory.
    pub fn from_stream(
        reader: impl AsyncRead + Send + Sync + Unpin + 'static,
        length: Option<u64>,
    ) -> Self {
        Self::Stream {
            reader: Box::new(reader),
            length,
        }
    }
}

/// Client for `/uploads/*` API endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Uploads<'a> {
    pub(crate) transport: &'a Transport,
}

impl<'a> Uploads<'a> {
    pub(crate) fn new(transport: &'a Transport) -> Self {
        Self { transport }
    }

    /// Starts building a request to upload a media file via `POST /uploads`. Requires authentication.
    ///
    /// The file is sent as a `multipart/form-data` payload under the part name `"file"`.
    /// Large files stream directly to the network without being fully buffered into memory.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # async fn doc_example(client: &actos::Actos) -> actos::Result<()> {
    /// // 1. Upload media attachment
    /// let upload = client
    ///     .uploads()
    ///     .create(std::path::Path::new("screenshot.png"))
    ///     .send()
    ///     .await?;
    ///
    /// // 2. Attach upload ID when creating a post
    /// let post = client
    ///     .posts()
    ///     .create("New Update", "Here is a screenshot of the new UI")
    ///     .attachment_ids([upload.id])
    ///     .send()
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn create(&self, source: impl Into<UploadSource>) -> CreateUploadBuilder<'a> {
        CreateUploadBuilder {
            transport: self.transport,
            source: source.into(),
            filename: None,
            mime_type: None,
        }
    }

    /// Deletes an upload via `DELETE /uploads/{id}`. Requires authentication.
    ///
    /// Expects 204 No Content.
    pub async fn delete(&self, id: &str) -> Result<()> {
        let path = format!("/uploads/{id}");
        let builder = self.transport.request(Method::DELETE, &path)?;
        self.transport.execute(builder).await?;
        Ok(())
    }
}

/// Builder for uploading a file via `POST /uploads`.
#[derive(Debug)]
#[must_use = "builders do nothing until .send().await is called"]
pub struct CreateUploadBuilder<'a> {
    transport: &'a Transport,
    source: UploadSource,
    filename: Option<String>,
    mime_type: Option<String>,
}

impl<'a> CreateUploadBuilder<'a> {
    /// Sets a custom filename to send in the multipart `Content-Disposition`.
    pub fn filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Sets the MIME type of the uploaded file (e.g. `"image/png"`, `"image/webp"`).
    pub fn mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Dispatches the multipart upload request and returns metadata of the stored media.
    pub async fn send(self) -> Result<UploadResponse> {
        let (mut part, default_filename) = match self.source {
            UploadSource::Path(path) => {
                let default_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(ToString::to_string);
                let meta = tokio::fs::metadata(&path).await?;
                let file = tokio::fs::File::open(&path).await?;
                let stream = tokio_util::io::ReaderStream::new(file);
                let body = reqwest::Body::wrap_stream(stream);
                let part = reqwest::multipart::Part::stream_with_length(body, meta.len());
                (part, default_name)
            }
            UploadSource::Bytes(bytes) => {
                let part = reqwest::multipart::Part::bytes(bytes);
                (part, None)
            }
            UploadSource::Stream { reader, length } => {
                let stream = tokio_util::io::ReaderStream::new(reader);
                let body = reqwest::Body::wrap_stream(stream);
                let part = if let Some(len) = length {
                    reqwest::multipart::Part::stream_with_length(body, len)
                } else {
                    reqwest::multipart::Part::stream(body)
                };
                (part, None)
            }
        };

        if let Some(name) = self.filename.or(default_filename) {
            part = part.file_name(name);
        }

        if let Some(mime) = self.mime_type {
            part = part
                .mime_str(&mime)
                .map_err(|e| Error::Config(e.to_string()))?;
        }

        let form = reqwest::multipart::Form::new().part("file", part);
        let builder = self
            .transport
            .request(Method::POST, "/uploads")?
            .multipart(form);
        self.transport.execute_json(builder).await
    }
}
