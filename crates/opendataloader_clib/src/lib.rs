use std::cell::RefCell;
use std::ffi::{CStr, CString, c_uint};
use std::os::raw::{c_char, c_int};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

use anyhow::Context;
pub use opendataloader_core::ConversionOptions;
use opendataloader_core::convert_with_options;

pub const ODL_STATUS_OK: c_int = 0;
pub const ODL_STATUS_INVALID_ARGUMENT: c_int = 1;
pub const ODL_STATUS_CONVERSION_ERROR: c_int = 2;
pub const ODL_STATUS_PANIC: c_int = 3;

pub const ODL_OPTION_JSON: c_uint = 1;
pub const ODL_OPTION_MARKDOWN: c_uint = 2;
pub const ODL_OPTION_IMAGES: c_uint = 4;
pub const ODL_OPTION_PARALLEL: c_uint = 8;

/// Convert local PDFs using the same file-writing pipeline as the C ABI.
pub fn convert_batch(pdf_paths: &[PathBuf], out_dir: &Path, options: ConversionOptions) -> anyhow::Result<()> {
    let out_dir = out_dir.to_path_buf();
    convert_with_options(pdf_paths, &out_dir, options)
        .with_context(|| format!("convert PDF batch to {}", out_dir.display()))
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

enum AbiError {
    InvalidArgument(String),
    Conversion(String),
}

fn set_last_error(message: impl Into<String>) {
    let message = message.into().replace('\0', "").into_bytes();
    LAST_ERROR.with(|error| *error.borrow_mut() = CString::new(message).ok());
}

fn clear_last_error() {
    LAST_ERROR.with(|error| *error.borrow_mut() = None);
}

#[unsafe(no_mangle)]
pub extern "C" fn odl_last_error() -> *const c_char {
    LAST_ERROR.with(|error| {
        error
            .borrow()
            .as_ref()
            .map_or(std::ptr::null(), |message| message.as_ptr())
    })
}

fn convert(
    pdf_paths: *const *const c_char,
    n_paths: c_uint,
    out_dir: *const c_char,
    mode: c_uint,
) -> Result<(), AbiError> {
    let known_options = ODL_OPTION_JSON | ODL_OPTION_MARKDOWN | ODL_OPTION_IMAGES | ODL_OPTION_PARALLEL;
    if pdf_paths.is_null() || n_paths == 0 {
        return Err(AbiError::InvalidArgument(
            "pdf_paths must be non-null and n_paths must be greater than zero".into(),
        ));
    }
    if out_dir.is_null() {
        return Err(AbiError::InvalidArgument("out_dir must be non-null".into()));
    }
    if mode & !known_options != 0 {
        return Err(AbiError::InvalidArgument(format!(
            "unsupported option bits: 0x{:x}",
            mode & !known_options
        )));
    }
    if mode & (ODL_OPTION_JSON | ODL_OPTION_MARKDOWN) == 0 {
        return Err(AbiError::InvalidArgument(
            "at least one output format option is required".into(),
        ));
    }

    let paths = unsafe { std::slice::from_raw_parts(pdf_paths, n_paths as usize) }
        .iter()
        .enumerate()
        .map(|(index, path)| {
            if path.is_null() {
                return Err(AbiError::InvalidArgument(format!(
                    "pdf_paths[{index}] must be non-null"
                )));
            }
            let path = unsafe { CStr::from_ptr(*path) }
                .to_str()
                .map_err(|_| AbiError::InvalidArgument(format!("pdf_paths[{index}] is not valid UTF-8")))?;
            if path.is_empty() {
                return Err(AbiError::InvalidArgument(format!(
                    "pdf_paths[{index}] must not be empty"
                )));
            }
            Ok(PathBuf::from(path))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let out_dir = unsafe { CStr::from_ptr(out_dir) }
        .to_str()
        .map_err(|_| AbiError::InvalidArgument("out_dir is not valid UTF-8".into()))?;
    if out_dir.is_empty() {
        return Err(AbiError::InvalidArgument("out_dir must not be empty".into()));
    }

    convert_batch(
        &paths,
        Path::new(out_dir),
        ConversionOptions {
            json_enabled: mode & ODL_OPTION_JSON != 0,
            markdown_enabled: mode & ODL_OPTION_MARKDOWN != 0,
            image_output_enabled: mode & ODL_OPTION_IMAGES != 0,
            parallel: mode & ODL_OPTION_PARALLEL != 0,
        },
    )
    .map_err(|error| AbiError::Conversion(format!("{error:#}")))
}

/// Convert a batch of local PDFs and write the selected output files.
#[unsafe(no_mangle)]
pub extern "C" fn odl_convert(
    pdf_paths: *const *const c_char,
    n_paths: c_uint,
    out_dir: *const c_char,
    mode: c_uint,
) -> c_int {
    clear_last_error();
    match panic::catch_unwind(AssertUnwindSafe(|| convert(pdf_paths, n_paths, out_dir, mode))) {
        Ok(Ok(())) => ODL_STATUS_OK,
        Ok(Err(AbiError::InvalidArgument(error))) => {
            set_last_error(error);
            ODL_STATUS_INVALID_ARGUMENT
        }
        Ok(Err(AbiError::Conversion(error))) => {
            set_last_error(error);
            ODL_STATUS_CONVERSION_ERROR
        }
        Err(_) => {
            set_last_error("conversion panicked");
            ODL_STATUS_PANIC
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn fixture() -> Result<CString, Box<dyn std::error::Error>> {
        Ok(CString::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../data/pdf/lorem.pdf")
                .to_string_lossy()
                .as_bytes(),
        )?)
    }

    #[test]
    fn converts_a_batch_and_exposes_errors() -> Result<(), Box<dyn std::error::Error>> {
        let pdf = fixture()?;
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/abi-test");
        let output_c = CString::new(output.to_string_lossy().as_bytes())?;
        let paths = [pdf.as_ptr()];

        assert_eq!(
            odl_convert(paths.as_ptr(), 1, output_c.as_ptr(), ODL_OPTION_JSON),
            ODL_STATUS_OK
        );
        assert!(output.join("lorem.json").is_file());
        assert_eq!(odl_last_error(), std::ptr::null());

        assert_eq!(
            odl_convert(std::ptr::null(), 1, output_c.as_ptr(), ODL_OPTION_JSON),
            ODL_STATUS_INVALID_ARGUMENT
        );
        let error = unsafe { CStr::from_ptr(odl_last_error()) }.to_str()?;
        assert!(error.contains("pdf_paths"));
        fs::remove_file(output.join("lorem.json"))?;
        Ok(())
    }

    #[test]
    fn rejects_unsupported_and_empty_modes() -> Result<(), Box<dyn std::error::Error>> {
        let pdf = fixture()?;
        let output = CString::new("target/abi-test")?;
        let paths = [pdf.as_ptr()];

        assert_eq!(
            odl_convert(paths.as_ptr(), 1, output.as_ptr(), 0),
            ODL_STATUS_INVALID_ARGUMENT
        );
        assert_eq!(
            odl_convert(paths.as_ptr(), 1, output.as_ptr(), 16),
            ODL_STATUS_INVALID_ARGUMENT
        );
        Ok(())
    }

    #[test]
    fn reports_missing_input_and_output_failures() -> Result<(), Box<dyn std::error::Error>> {
        let output = CString::new("target/abi-error-test")?;
        let missing = CString::new("data/pdf/missing.pdf")?;
        let paths = [missing.as_ptr()];
        assert_eq!(
            odl_convert(paths.as_ptr(), 1, output.as_ptr(), ODL_OPTION_JSON),
            ODL_STATUS_CONVERSION_ERROR
        );
        let error = unsafe { CStr::from_ptr(odl_last_error()) }.to_str()?;
        assert!(error.contains("missing.pdf"), "{error}");

        let output_path = PathBuf::from("target/abi-output-file");
        fs::write(&output_path, b"not a directory")?;
        let pdf = fixture()?;
        let paths = [pdf.as_ptr()];
        let output = CString::new(output_path.to_string_lossy().as_bytes())?;
        assert_eq!(
            odl_convert(paths.as_ptr(), 1, output.as_ptr(), ODL_OPTION_JSON),
            ODL_STATUS_CONVERSION_ERROR
        );
        fs::remove_file(output_path)?;
        Ok(())
    }

    #[test]
    fn writes_images_and_processes_parallel_batches() -> Result<(), Box<dyn std::error::Error>> {
        let first = fixture()?;
        let second = CString::new(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../data/pdf/chinese_scan.pdf")
                .to_string_lossy()
                .as_bytes(),
        )?;
        let paths = [first.as_ptr(), second.as_ptr()];
        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/abi-batch-test");
        let output_c = CString::new(output.to_string_lossy().as_bytes())?;
        assert_eq!(
            odl_convert(
                paths.as_ptr(),
                2,
                output_c.as_ptr(),
                ODL_OPTION_JSON | ODL_OPTION_IMAGES | ODL_OPTION_PARALLEL
            ),
            ODL_STATUS_OK
        );
        assert!(output.join("lorem.json").is_file());
        assert!(output.join("chinese_scan.json").is_file());
        assert!(output.join("chinese_scan_images/imageFile1.png").is_file());
        Ok(())
    }
}
