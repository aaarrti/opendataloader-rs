use std::{fs, path::Path};

use lopdf::Document as PdfDocument;

use super::model::*;

pub fn write_external_images(
    pdf_path: &Path,
    image_dir: &Path,
    document: &mut Document,
    format: &str,
) -> anyhow::Result<()> {
    let pdf = PdfDocument::load(pdf_path)
        .map_err(|error| anyhow::anyhow!("load image source PDF {}: {error}", pdf_path.display()))?;
    let extension = match format {
        "png" => "png",
        "jpeg" => "jpeg",
        other => anyhow::bail!("unsupported image format: {other}"),
    };
    let prefix = image_dir.file_name().and_then(|name| name.to_str()).unwrap_or("images");
    let image_count = document
        .pages
        .iter()
        .flat_map(|page| page.chunks.iter())
        .filter(|chunk| matches!(chunk, ParserChunk::Image(_)))
        .count();
    if image_count == 0 {
        return Ok(());
    }
    fs::create_dir_all(image_dir)?;

    let mut image_index = 0;
    for page in &document.pages {
        for chunk in &page.chunks {
            let ParserChunk::Image(image) = chunk else { continue };
            image_index += 1;
            let file_name = format!("imageFile{image_index}.{extension}");
            let destination = image_dir.join(&file_name);
            let written = if let Some(inline) = &image.inline_image {
                extension == "png"
                    && write_png_data(inline.width, inline.height, inline.channels, &inline.data, &destination).is_ok()
            } else {
                let Some(object_name) = image.object_reference.as_deref() else {
                    continue;
                };
                let Some(stream) = image_stream(&pdf, page.index, object_name) else {
                    continue;
                };
                match extension {
                    "jpeg" => {
                        stream
                            .filters()
                            .is_ok_and(|filters| filters.iter().any(|filter| filter == b"DCTDecode"))
                            && fs::write(&destination, &stream.content).is_ok()
                    }
                    _ => write_png_stream(stream, &destination).is_ok(),
                }
            };
            if !written {
                continue;
            }
            if let Some(reference) = document.elements.iter_mut().find_map(|element| match element {
                SemanticElement::Image { reference, .. } if reference.source.is_none() => Some(reference),
                _ => None,
            }) {
                reference.source = Some(format!("{prefix}/{file_name}"));
                reference.format = Some(extension.into());
            }
        }
    }
    Ok(())
}

fn image_stream<'a>(pdf: &'a PdfDocument, page_index: usize, name: &str) -> Option<&'a lopdf::Stream> {
    let page_id = *pdf.get_pages().get(&((page_index + 1) as u32))?;
    let (resources, _) = pdf.get_page_resources(page_id).ok()?;
    let xobjects = resources?.get_deref(b"XObject", pdf).ok()?.as_dict().ok()?;
    let reference = xobjects.get(name.as_bytes()).ok()?.as_reference().ok()?;
    let stream = pdf.get_object(reference).ok()?.as_stream().ok()?;
    (stream.dict.get(b"Subtype").ok()?.as_name().ok()? == b"Image").then_some(stream)
}

fn write_png_stream(stream: &lopdf::Stream, destination: &Path) -> anyhow::Result<()> {
    let width = stream.dict.get(b"Width")?.as_i64()? as u32;
    let height = stream.dict.get(b"Height")?.as_i64()? as u32;
    let channels = match stream.dict.get(b"ColorSpace")?.as_name()? {
        b"DeviceGray" => 1u8,
        b"DeviceRGB" => 3u8,
        _ => anyhow::bail!("unsupported PDF image color space"),
    };
    if stream.dict.get(b"BitsPerComponent")?.as_i64()? != 8 {
        anyhow::bail!("unsupported PDF image bit depth");
    }
    let data = stream.decompressed_content_with_limit((width as usize) * (height as usize) * channels as usize)?;
    write_png_data(width, height, channels, &data, destination)
}

fn write_png_data(width: u32, height: u32, channels: u8, data: &[u8], destination: &Path) -> anyhow::Result<()> {
    let file = fs::File::create(destination)?;
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(if channels == 1 {
        png::ColorType::Grayscale
    } else {
        png::ColorType::Rgb
    });
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn external_images_write_png_and_reference_written_file() -> anyhow::Result<()> {
        let pdf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/chinese_scan.pdf");
        let image_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/i1-test/images");
        fs::create_dir_all(&image_dir)?;
        let mut document = crate::parse_pdf(&pdf_path).map_err(|error| anyhow::anyhow!(error))?;
        write_external_images(&pdf_path, &image_dir, &mut document, "png")?;
        let image_path = image_dir.join("imageFile1.png");
        assert!(image_path.is_file());
        assert_eq!(fs::read(&image_path)?.get(..8), Some(b"\x89PNG\r\n\x1a\n".as_slice()));
        assert!(
            matches!(document.elements.first(), Some(SemanticElement::Image { reference, .. }) if reference.source.as_deref() == Some("images/imageFile1.png") && reference.format.as_deref() == Some("png"))
        );
        Ok(())
    }

    #[test]
    fn external_images_write_supported_inline_image() -> anyhow::Result<()> {
        let pdf_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/lorem.pdf");
        let mut document = Document {
            file_name: "inline.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page {
                index: 0,
                width: 1.0,
                height: 1.0,
                chunks: vec![ParserChunk::Image(ImageChunk {
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 0.0,
                        bottom: 0.0,
                        right: 1.0,
                        top: 1.0,
                    },
                    object_reference: None,
                    inline_image: Some(InlineImage {
                        width: 1,
                        height: 1,
                        channels: 3,
                        data: vec![255, 0, 0],
                    }),
                    parser_order: 0,
                    structure_id: None,
                    pdfua_tag: None,
                })],
            }],
            elements: vec![SemanticElement::Image {
                common: ElementCommon {
                    id: Some(1),
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 0.0,
                        bottom: 0.0,
                        right: 1.0,
                        top: 1.0,
                    },
                    pdfua_tag: None,
                },
                reference: ImageReference {
                    source: None,
                    data: None,
                    format: None,
                },
            }],
        };
        let image_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/inline-image-test/images");
        write_external_images(&pdf_path, &image_dir, &mut document, "png")?;
        assert!(image_dir.join("imageFile1.png").is_file());
        Ok(())
    }
}
