pub mod detect;
pub mod download;
pub mod executor;
pub mod rewriter;

pub use download::download_installer;
pub use executor::InstallerExecutor;
pub use rewriter::InstallerRewriter;
