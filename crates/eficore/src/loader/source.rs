use crate::path::ResolvedPath;
use crate::shim::ShimInput;

/// Represents a source of an EFI image.
pub enum ImageSource<'source> {
    /// The image is located at the specified path that has been resolved.
    ResolvedPath(&'source ResolvedPath),
    /// The image is located in a buffer.
    DataBuffer {
        /// Optional path to the image.
        path: Option<&'source ResolvedPath>,
        /// Buffer containing the image.
        buffer: &'source [u8],
    },
}

/// Implement conversion from `ImageSource` to `ShimInput`, which is used by the shim support code.
impl<'source> From<ImageSource<'source>> for ShimInput<'source> {
    fn from(value: ImageSource<'source>) -> Self {
        match value {
            ImageSource::ResolvedPath(path) => ShimInput::ResolvedPath(path),
            ImageSource::DataBuffer { path, buffer } => ShimInput::DataBuffer(path, buffer),
        }
    }
}
