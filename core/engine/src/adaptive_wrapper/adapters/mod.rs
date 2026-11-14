pub mod bittorrent;
pub mod ed2k;
pub mod ffmpeg;
pub mod ftp;
pub mod github;
pub mod http;
pub mod huggingface;
pub mod m3u8;
pub mod ytdlp;

pub use bittorrent::BitTorrentAdapter;
pub use ed2k::Ed2kAdapter;
pub use ffmpeg::FfmpegAdapter;
pub use ftp::FtpAdapter;
pub use github::GithubAdapter;
pub use http::HttpAdapter;
pub use huggingface::HuggingFaceAdapter;
pub use m3u8::M3u8Adapter;
pub use ytdlp::YtDlpAdapter;

use super::traits::AdaptiveWrapperAdapter;
use crate::packs::FeaturePackId;

pub fn get_adapter(pack_id: FeaturePackId) -> Box<dyn AdaptiveWrapperAdapter> {
    match pack_id {
        FeaturePackId::HttpPack => Box::new(HttpAdapter),
        FeaturePackId::BitTorrentPack => Box::new(BitTorrentAdapter),
        FeaturePackId::FtpPack => Box::new(FtpAdapter),
        FeaturePackId::Ed2kPack => Box::new(Ed2kAdapter),
        FeaturePackId::M3u8Pack => Box::new(M3u8Adapter),
        FeaturePackId::FfmpegPack => Box::new(FfmpegAdapter),
        FeaturePackId::YtDlpPack => Box::new(YtDlpAdapter),
        FeaturePackId::GithubPack => Box::new(GithubAdapter),
        FeaturePackId::HuggingFacePack => Box::new(HuggingFaceAdapter),
    }
}
