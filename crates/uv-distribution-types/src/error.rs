use uv_normalize::PackageName;
use uv_redacted::DisplaySafeUrl;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),

    #[error(transparent)]
    WheelFilename(#[from] uv_distribution_filename::WheelFilenameError),

    #[error("Could not extract path segments from URL: {0}")]
    MissingPathSegments(String),

    #[error("Distribution not found at: {0}")]
    NotFound(DisplaySafeUrl),

    #[error("Requested package name `{0}` does not match `{1}` in the distribution filename: {2}")]
    PackageNameMismatch(PackageName, PackageName, String),
}
