use crate::cli::FileOverlayArgs;
use crate::error::AppError;

pub fn run(args: FileOverlayArgs) -> Result<String, AppError> {
    let pretty = args.common.pretty;
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let output = crate::overlay::file_overlay::build_file_overlay(&repo, args)?;
    crate::serialize_json(&output, pretty)
}
