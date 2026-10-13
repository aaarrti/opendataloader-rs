use anyhow::{Result, bail};
use clap::Parser;
use std::path::PathBuf;

use opendataloader_clib::{ConversionOptions, convert_batch};

#[derive(Debug, Parser)]
#[command(name = "opendataloader", about = "Convert local PDF files to JSON and/or Markdown")]
struct CliArg {
    /// One or more local PDF files. Directories are not traversed.
    #[arg(long = "input-paths", required = true, num_args = 1.., value_name = "PDF")]
    input_paths: Vec<PathBuf>,
    /// Directory for generated .json, .md, and image files.
    #[arg(long = "out-dir", required = true, value_name = "DIR")]
    out_dir: PathBuf,
    /// Write JSON output (disabled by default).
    #[arg(long)]
    json: bool,
    /// Write Markdown output (disabled by default).
    #[arg(long)]
    markdown: bool,
    /// Write external PNG image files for extracted images.
    #[arg(long)]
    image: bool,
    /// Process multiple PDFs concurrently (opt-in; single PDFs remain serial).
    #[arg(long)]
    parallel: bool,
}

fn main() -> anyhow::Result<()> {
    run(CliArg::parse())
}

fn run(cli_args: CliArg) -> Result<()> {
    if !cli_args.json && !cli_args.markdown {
        bail!("at least one of --json or --markdown is required");
    }
    if cli_args.image && !cli_args.json && !cli_args.markdown {
        bail!("--image requires --json or --markdown");
    }
    if let Some(path) = cli_args.input_paths.iter().find(|path| path.is_dir()) {
        bail!(
            "input path is a directory; recursive traversal is not supported: {}",
            path.display()
        );
    }

    convert_batch(
        &cli_args.input_paths,
        &cli_args.out_dir,
        ConversionOptions {
            json_enabled: cli_args.json,
            markdown_enabled: cli_args.markdown,
            image_output_enabled: cli_args.image,
            parallel: cli_args.parallel,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use std::path::Path;

    fn args(extra: &[&str]) -> std::result::Result<CliArg, clap::Error> {
        let mut values = vec!["opendataloader", "--input-paths", "input.pdf", "--out-dir", "out"];
        values.extend_from_slice(extra);
        CliArg::try_parse_from(values)
    }

    #[test]
    fn requires_an_output_format() -> Result<()> {
        let parsed = args(&[])?;
        assert!(run(parsed).is_err());
        Ok(())
    }

    #[test]
    fn accepts_one_or_more_inputs_and_parallel() -> Result<()> {
        let parsed = CliArg::try_parse_from([
            "opendataloader",
            "--input-paths",
            "input.pdf",
            "second.pdf",
            "--out-dir",
            "out",
            "--json",
            "--parallel",
        ])?;
        assert_eq!(parsed.input_paths.len(), 2);
        assert!(parsed.parallel);
        Ok(())
    }

    #[test]
    fn rejects_directories_without_traversing() -> Result<()> {
        let parsed = CliArg {
            input_paths: vec![Path::new(".").to_path_buf()],
            out_dir: PathBuf::from("out"),
            json: true,
            markdown: false,
            image: false,
            parallel: false,
        };
        assert!(run(parsed).is_err());
        Ok(())
    }

    #[test]
    fn reports_conversion_failures() -> Result<()> {
        let parsed = args(&["--json"])?;
        assert!(run(parsed).is_err());
        Ok(())
    }

    #[test]
    fn writes_selected_outputs_and_images() -> Result<()> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let input = root.join("../../data/pdf/chinese_scan.pdf");
        let output = root.join("../../target/cli-test");
        run(CliArg {
            input_paths: vec![input],
            out_dir: output.clone(),
            json: true,
            markdown: true,
            image: true,
            parallel: false,
        })?;
        assert!(output.join("chinese_scan.json").is_file());
        assert!(output.join("chinese_scan.md").is_file());
        assert!(output.join("chinese_scan_images/imageFile1.png").is_file());
        Ok(())
    }
}
