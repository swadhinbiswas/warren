pub mod download;
pub mod rewriter;
pub mod executor;
pub mod detect;

pub use download::download_installer;
pub use rewriter::InstallerRewriter;
pub use executor::InstallerExecutor;
pub use detect::detect_binary;
