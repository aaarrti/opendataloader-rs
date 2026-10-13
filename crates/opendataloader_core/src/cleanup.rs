use super::model::*;

pub fn normalize_page_chunks(page: &mut Page) {
    let mut seen_text = Vec::new();
    page.chunks.retain_mut(|chunk| match chunk {
        ParserChunk::Text(text) => {
            text.text = clean_text(&text.text);
            let duplicate = seen_text
                .iter()
                .any(|(value, bounds): &(String, BoundingBox)| value == &text.text && *bounds == text.bounds);
            let keep = !text.text.is_empty()
                && text.font.size.is_none_or(|size| size >= 1.0)
                && intersects_page(text.bounds, page.width, page.height)
                && !duplicate;
            if keep {
                seen_text.push((text.text.clone(), text.bounds));
            }
            keep
        }
        ParserChunk::Image(image) => intersects_page(image.bounds, page.width, page.height),
        ParserChunk::LineArt(line) => {
            intersects_page(line.bounds, page.width, page.height)
                && !is_page_background(line.bounds, page.width, page.height)
        }
    });
}

fn clean_text(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character == '\0' || character == '\u{fffd}' {
                ' '
            } else {
                character
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn intersects_page(bounds: BoundingBox, width: f64, height: f64) -> bool {
    bounds.right > 0.0 && bounds.top > 0.0 && bounds.left < width && bounds.bottom < height
}

fn is_page_background(bounds: BoundingBox, width: f64, height: f64) -> bool {
    bounds.left <= 0.0 && bounds.bottom <= 0.0 && bounds.right >= width && bounds.top >= height
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parser_applies_fixture_text_cleanup() -> anyhow::Result<()> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../data/pdf/invalid-chars.pdf");
        let document = crate::parse_pdf(&path).map_err(|error| anyhow::anyhow!(error))?;
        let text = document.pages[0]
            .chunks
            .iter()
            .find_map(|chunk| match chunk {
                ParserChunk::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .ok_or_else(|| anyhow::anyhow!("invalid-chars fixture has no text chunk"))?;
        assert!(!text.contains('\u{fffd}'));
        assert!(!text.contains("  "));
        Ok(())
    }

    #[test]
    fn cleanup_removes_default_noise_and_normalizes_text() {
        let bounds = BoundingBox {
            left: 10.0,
            bottom: 10.0,
            right: 100.0,
            top: 30.0,
        };
        let mut page = Page {
            index: 0,
            width: 100.0,
            height: 100.0,
            chunks: vec![
                ParserChunk::Text(TextChunk {
                    page_index: 0,
                    bounds,
                    text: "  hello\0  world\u{fffd} ".into(),
                    glyph_order: Vec::new(),
                    character_spacing: None,
                    font: FontInfo::default(),
                    parser_order: 0,
                    structure_id: None,
                    pdfua_tag: None,
                }),
                ParserChunk::Text(TextChunk {
                    page_index: 0,
                    bounds,
                    text: "hello world ".into(),
                    glyph_order: Vec::new(),
                    character_spacing: None,
                    font: FontInfo::default(),
                    parser_order: 1,
                    structure_id: None,
                    pdfua_tag: None,
                }),
                ParserChunk::Text(TextChunk {
                    page_index: 0,
                    bounds,
                    text: "tiny".into(),
                    glyph_order: Vec::new(),
                    character_spacing: None,
                    font: FontInfo {
                        size: Some(0.5),
                        ..Default::default()
                    },
                    parser_order: 2,
                    structure_id: None,
                    pdfua_tag: None,
                }),
                ParserChunk::Text(TextChunk {
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 120.0,
                        bottom: 10.0,
                        right: 130.0,
                        top: 20.0,
                    },
                    text: "outside".into(),
                    glyph_order: Vec::new(),
                    character_spacing: None,
                    font: FontInfo::default(),
                    parser_order: 3,
                    structure_id: None,
                    pdfua_tag: None,
                }),
                ParserChunk::LineArt(LineArtChunk {
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 0.0,
                        bottom: 0.0,
                        right: 100.0,
                        top: 100.0,
                    },
                    parser_order: 4,
                }),
            ],
        };
        normalize_page_chunks(&mut page);
        assert_eq!(page.chunks.len(), 1);
        assert!(matches!(&page.chunks[0], ParserChunk::Text(text) if text.text == "hello world"));
    }

    #[test]
    fn cleanup_does_not_apply_sensitive_data_sanitization() {
        let mut page = Page {
            index: 0,
            width: 100.0,
            height: 100.0,
            chunks: vec![ParserChunk::Text(TextChunk {
                page_index: 0,
                bounds: BoundingBox {
                    left: 0.0,
                    bottom: 0.0,
                    right: 100.0,
                    top: 100.0,
                },
                text: "alice@example.org".into(),
                glyph_order: Vec::new(),
                character_spacing: None,
                font: FontInfo::default(),
                parser_order: 0,
                structure_id: None,
                pdfua_tag: None,
            })],
        };
        normalize_page_chunks(&mut page);
        assert!(matches!(&page.chunks[0], ParserChunk::Text(text) if text.text == "alice@example.org"));
    }
}
