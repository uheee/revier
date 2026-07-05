use crate::cli::FileOverlayArgs;
use crate::error::AppError;
use crate::json::FileOverlayCommandOutput;

pub fn analyze(args: FileOverlayArgs) -> Result<FileOverlayCommandOutput, AppError> {
    file_overlay_output(args)
}

pub fn run(args: FileOverlayArgs) -> Result<String, AppError> {
    let pretty = args.common.pretty;
    let output = analyze(args)?;
    crate::serialize_json(&output, pretty)
}

fn file_overlay_output(args: FileOverlayArgs) -> Result<FileOverlayCommandOutput, AppError> {
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    crate::overlay::file_overlay::build_file_overlay(&repo, args)
}
