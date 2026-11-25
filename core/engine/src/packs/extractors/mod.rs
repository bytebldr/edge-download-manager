//! Native Media Site Extractor Trait & Registry
//! Allows YouTube, Vimeo, and additional sites to be extracted in-process
//! without external yt-dlp dependencies.

pub mod types;
pub mod vimeo;
pub mod youtube;

pub use types::{ExtractedFormat, ExtractedMetadata, SiteExtractor};
pub use vimeo::VimeoExtractor;
pub use youtube::YouTubeExtractor;
