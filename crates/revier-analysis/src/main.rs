fn main() {
    match run_cli() {
        Ok(output) => print!("{output}"),
        Err(error) => {
            eprintln!("{error:#}");
            let exit_code = error
                .downcast_ref::<revier_analysis::error::AppError>()
                .map_or(1, revier_analysis::error::AppError::exit_code);
            std::process::exit(exit_code);
        }
    }
}

fn run_cli() -> anyhow::Result<String> {
    revier_analysis::run_from(std::env::args_os())
        .map_err(|error| anyhow::Error::new(error).context("分析 CLI 执行失败"))
}
