use super::model::*;

pub fn reconstruct_semantics(document: &mut Document) {
    document.elements.clear();
    for page in &document.pages {
        document.elements.extend(page.chunks.iter().filter_map(|chunk| {
            let ParserChunk::Image(image) = chunk else { return None };
            Some(SemanticElement::Image {
                common: ElementCommon {
                    id: None,
                    page_index: image.page_index,
                    bounds: image.bounds,
                    pdfua_tag: image.pdfua_tag.clone(),
                },
                reference: ImageReference {
                    source: None,
                    data: None,
                    format: None,
                },
            })
        }));
        let tables = tables_from_page(page);
        let table_bounds = tables.iter().map(element_bounds).collect::<Vec<_>>();
        for table in tables {
            document.elements.push(table);
        }
        let lines = text_lines(page)
            .into_iter()
            .filter(|line| {
                !table_bounds
                    .iter()
                    .any(|bounds| contains_bounds(*bounds, line.common.bounds))
            })
            .collect::<Vec<_>>();
        let mut index = 0;
        while index < lines.len() {
            if let Some((style, _)) = tagged_list(&lines[index]).or_else(|| list_marker(&lines[index].text)) {
                let start = index;
                let mut items = Vec::new();
                while index < lines.len() {
                    let Some((item_style, item_text)) =
                        tagged_list(&lines[index]).or_else(|| list_marker(&lines[index].text))
                    else {
                        break;
                    };
                    if item_style != style {
                        break;
                    }
                    let line = &lines[index];
                    items.push(SemanticElement::ListItem {
                        common: line.common.clone(),
                        children: vec![SemanticElement::Paragraph {
                            common: line.common.clone(),
                            text: item_text,
                            font: line.font.clone(),
                        }],
                        text: None,
                    });
                    index += 1;
                }
                let common = common_for_lines(&lines[start..index]);
                document.elements.push(SemanticElement::List { common, style, items });
                continue;
            }

            let start = index;
            index += 1;
            while index < lines.len()
                && same_structure_tag(&lines[index - 1], &lines[index])
                && joins_paragraph(&lines[index - 1], &lines[index])
            {
                index += 1;
            }
            let group = &lines[start..index];
            let common = common_for_lines(group);
            let text = group
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            let font = group[0].font.clone();
            if let Some(caption) = tagged_caption(group) {
                document.elements.push(SemanticElement::Caption {
                    common,
                    text: caption,
                    linked_content_id: None,
                });
            } else if let Some(level) = tagged_heading_level(group).or_else(|| {
                group
                    .first()
                    .filter(|line| line.common.pdfua_tag.is_none())
                    .and_then(|_| heading_level(group))
            }) {
                document.elements.push(SemanticElement::Heading {
                    common,
                    level,
                    text,
                    font,
                });
            } else {
                document
                    .elements
                    .push(SemanticElement::Paragraph { common, text, font });
            }
        }
    }
}

fn tables_from_page(page: &Page) -> Vec<SemanticElement> {
    let epsilon = 1.0;
    let lines = page
        .chunks
        .iter()
        .filter_map(|chunk| match chunk {
            ParserChunk::LineArt(line)
                if !is_page_background(page, line.bounds, epsilon)
                    && !is_page_edge_line(page, line.bounds, epsilon)
                    && is_table_line(line.bounds) =>
            {
                Some(line.bounds)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut groups: Vec<Vec<BoundingBox>> = Vec::new();
    for line in lines {
        let mut matches = Vec::new();
        for (index, group) in groups.iter().enumerate() {
            if group.iter().any(|other| touches(*other, line, epsilon)) {
                matches.push(index);
            }
        }
        if let Some(&first) = matches.first() {
            groups[first].push(line);
            for &index in matches.iter().skip(1).rev() {
                let merged = groups.remove(index);
                groups[first].extend(merged);
            }
        } else {
            groups.push(vec![line]);
        }
    }

    let text_lines = positioned_text_lines(page);
    groups
        .into_iter()
        .filter_map(|group| table_from_lines(page, &text_lines, &group))
        .collect()
}

fn is_table_line(bounds: BoundingBox) -> bool {
    bounds.right - bounds.left <= 2.0 || bounds.top - bounds.bottom <= 2.0
}

fn is_page_background(page: &Page, bounds: BoundingBox, epsilon: f64) -> bool {
    bounds.left <= epsilon
        && bounds.bottom <= epsilon
        && bounds.right >= page.width - epsilon
        && bounds.top >= page.height - epsilon
}

fn is_page_edge_line(page: &Page, bounds: BoundingBox, epsilon: f64) -> bool {
    let horizontal = bounds.top - bounds.bottom <= 2.0;
    let vertical = bounds.right - bounds.left <= 2.0;
    (horizontal && bounds.left <= epsilon && bounds.right >= page.width - epsilon)
        || (vertical && bounds.bottom <= epsilon && bounds.top >= page.height - epsilon)
}

fn table_from_lines(page: &Page, text_lines: &[TextLine], lines: &[BoundingBox]) -> Option<SemanticElement> {
    let mut columns = Vec::new();
    let mut rows = Vec::new();
    for line in lines {
        let width = line.right - line.left;
        let height = line.top - line.bottom;
        if width <= 0.01 && height > 0.01 {
            columns.push(line.left);
        } else if height <= 0.01 && width > 0.01 {
            rows.push(line.bottom);
        } else if width > 0.01 && height > 0.01 {
            columns.extend([line.left, line.right]);
            rows.extend([line.bottom, line.top]);
        }
    }
    deduplicate_coordinates(&mut columns);
    deduplicate_coordinates(&mut rows);
    if columns.len() < 2
        || rows.len() < 2
        || (columns.len() == 2 && rows.len() == 2)
        || (columns.len() - 1) * (rows.len() - 1) < 2
    {
        return None;
    }
    let bounds = BoundingBox {
        left: *columns.first()?,
        bottom: *rows.first()?,
        right: *columns.last()?,
        top: *rows.last()?,
    };
    if !text_lines
        .iter()
        .any(|line| contains_bounds(bounds, line.common.bounds))
    {
        return None;
    }
    let mut table_rows = Vec::new();
    for (row_index, pair) in rows.windows(2).rev().enumerate() {
        let row_bounds = BoundingBox {
            left: bounds.left,
            bottom: pair[0],
            right: bounds.right,
            top: pair[1],
        };
        let mut cells = Vec::new();
        for (column_index, pair) in columns.windows(2).enumerate() {
            let cell_bounds = BoundingBox {
                left: pair[0],
                bottom: row_bounds.bottom,
                right: pair[1],
                top: row_bounds.top,
            };
            let children = text_lines
                .iter()
                .filter(|line| contains_bounds(cell_bounds, line.common.bounds))
                .map(|line| SemanticElement::Paragraph {
                    common: line.common.clone(),
                    text: line.text.clone(),
                    font: line.font.clone(),
                })
                .collect();
            cells.push(SemanticElement::TableCell {
                common: ElementCommon {
                    id: None,
                    page_index: page.index,
                    bounds: cell_bounds,
                    pdfua_tag: Some(if row_index == 0 { "TH" } else { "TD" }.into()),
                },
                row_number: row_index + 1,
                column_number: column_index + 1,
                row_span: 1,
                column_span: 1,
                is_header: row_index == 0,
                children,
            });
        }
        table_rows.push(SemanticElement::TableRow {
            common: ElementCommon {
                id: None,
                page_index: page.index,
                bounds: row_bounds,
                pdfua_tag: Some("TR".into()),
            },
            row_number: row_index + 1,
            cells,
        });
    }
    Some(SemanticElement::Table {
        common: ElementCommon {
            id: None,
            page_index: page.index,
            bounds,
            pdfua_tag: Some("Table".into()),
        },
        rows: table_rows,
    })
}

fn touches(left: BoundingBox, right: BoundingBox, epsilon: f64) -> bool {
    left.left <= right.right + epsilon
        && right.left <= left.right + epsilon
        && left.bottom <= right.top + epsilon
        && right.bottom <= left.top + epsilon
}

fn deduplicate_coordinates(values: &mut Vec<f64>) {
    values.sort_by(f64::total_cmp);
    values.dedup_by(|left, right| (*left - *right).abs() <= 4.0);
}

fn positioned_text_lines(page: &Page) -> Vec<TextLine> {
    page.chunks
        .iter()
        .filter_map(|chunk| match chunk {
            ParserChunk::Text(text) => Some(TextLine {
                common: ElementCommon {
                    id: None,
                    page_index: text.page_index,
                    bounds: text.bounds,
                    pdfua_tag: text.pdfua_tag.clone(),
                },
                text: text.text.clone(),
                font: text.font.clone(),
            }),
            _ => None,
        })
        .collect()
}

pub(crate) fn element_bounds(element: &SemanticElement) -> BoundingBox {
    match element {
        SemanticElement::Table { common, .. }
        | SemanticElement::TableRow { common, .. }
        | SemanticElement::TableCell { common, .. }
        | SemanticElement::Heading { common, .. }
        | SemanticElement::Paragraph { common, .. }
        | SemanticElement::TextChunk { common, .. }
        | SemanticElement::TextBlock { common, .. }
        | SemanticElement::Formula { common, .. }
        | SemanticElement::Image { common, .. }
        | SemanticElement::Caption { common, .. }
        | SemanticElement::List { common, .. }
        | SemanticElement::ListItem { common, .. }
        | SemanticElement::Toc { common, .. }
        | SemanticElement::TocItem { common, .. } => common.bounds,
    }
}

pub(crate) fn contains_bounds(outer: BoundingBox, inner: BoundingBox) -> bool {
    inner.left >= outer.left && inner.right <= outer.right && inner.bottom >= outer.bottom && inner.top <= outer.top
}

#[derive(Debug, Clone)]
struct TextLine {
    common: ElementCommon,
    text: String,
    font: FontInfo,
}

fn text_lines(page: &Page) -> Vec<TextLine> {
    let mut lines = Vec::new();
    for chunk in page.chunks.iter().filter_map(|chunk| match chunk {
        ParserChunk::Text(text) => Some(text),
        _ => None,
    }) {
        if let Some(line) = lines.iter_mut().find(|line: &&mut TextLine| {
            line.common.pdfua_tag == chunk.pdfua_tag && same_line(&line.common.bounds, &chunk.bounds)
        }) {
            line.text.push_str(&chunk.text);
            line.common.bounds = union_bounds(line.common.bounds, chunk.bounds);
        } else {
            lines.push(TextLine {
                common: ElementCommon {
                    id: None,
                    page_index: chunk.page_index,
                    bounds: chunk.bounds,
                    pdfua_tag: chunk.pdfua_tag.clone(),
                },
                text: chunk.text.clone(),
                font: chunk.font.clone(),
            });
        }
    }
    lines.sort_by(|left, right| {
        right
            .common
            .bounds
            .top
            .total_cmp(&left.common.bounds.top)
            .then_with(|| left.common.bounds.left.total_cmp(&right.common.bounds.left))
    });
    lines
}

fn same_line(left: &BoundingBox, right: &BoundingBox) -> bool {
    let tolerance = (left.top - left.bottom).max(right.top - right.bottom) * 0.5;
    ((left.top + left.bottom) - (right.top + right.bottom)).abs() <= tolerance
}

fn joins_paragraph(previous: &TextLine, current: &TextLine) -> bool {
    let gap = previous.common.bounds.bottom - current.common.bounds.top;
    let left_delta = (previous.common.bounds.left - current.common.bounds.left).abs();
    previous.common.page_index == current.common.page_index
        && previous.font.size == current.font.size
        && left_delta <= previous.font.size.unwrap_or(12.0) * 0.25
        && (0.0..=previous.font.size.unwrap_or(12.0) * 2.5).contains(&gap)
}

fn heading_level(lines: &[TextLine]) -> Option<u8> {
    let size = lines.first()?.font.size?;
    let text_len = lines.iter().map(|line| line.text.chars().count()).sum::<usize>();
    (size >= 14.0 && text_len <= 80).then(|| ((24.0 - size) / 2.0).round().clamp(1.0, 6.0) as u8)
}

fn same_structure_tag(left: &TextLine, right: &TextLine) -> bool {
    left.common.pdfua_tag == right.common.pdfua_tag
}

fn tagged_heading_level(lines: &[TextLine]) -> Option<u8> {
    let tag = lines.first()?.common.pdfua_tag.as_deref()?;
    tag.strip_prefix('H')?
        .parse()
        .ok()
        .filter(|level| (1..=6).contains(level))
}

fn tagged_caption(lines: &[TextLine]) -> Option<String> {
    (lines.first()?.common.pdfua_tag.as_deref() == Some("Caption")).then(|| {
        lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn tagged_list(line: &TextLine) -> Option<(ListStyle, String)> {
    let tag = line.common.pdfua_tag.as_deref()?;
    matches!(tag, "L" | "LI").then(|| (ListStyle::Unordered, line.text.clone()))
}

fn list_marker(text: &str) -> Option<(ListStyle, String)> {
    let trimmed = text.trim_start();
    let (style, rest) = if let Some(rest) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        (ListStyle::Unordered, rest)
    } else {
        let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0
            || trimmed.as_bytes().get(digits) != Some(&b'.')
            || trimmed.as_bytes().get(digits + 1) != Some(&b' ')
        {
            return None;
        }
        (ListStyle::Ordered, &trimmed[digits + 2..])
    };
    (!rest.is_empty()).then(|| (style, rest.to_owned()))
}

fn common_for_lines(lines: &[TextLine]) -> ElementCommon {
    let first = &lines[0].common;
    ElementCommon {
        id: None,
        page_index: first.page_index,
        bounds: lines
            .iter()
            .skip(1)
            .fold(first.bounds, |bounds, line| union_bounds(bounds, line.common.bounds)),
        pdfua_tag: first.pdfua_tag.clone(),
    }
}

fn union_bounds(left: BoundingBox, right: BoundingBox) -> BoundingBox {
    BoundingBox {
        left: left.left.min(right.left),
        bottom: left.bottom.min(right.bottom),
        right: left.right.max(right.right),
        top: left.top.max(right.top),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str, top: f64, size: f64) -> ParserChunk {
        ParserChunk::Text(TextChunk {
            page_index: 0,
            bounds: BoundingBox {
                left: 50.0,
                bottom: top - 10.0,
                right: 250.0,
                top,
            },
            text: text.into(),
            glyph_order: Vec::new(),
            character_spacing: None,
            font: FontInfo {
                size: Some(size),
                ..Default::default()
            },
            parser_order: 0,
            structure_id: None,
            pdfua_tag: None,
        })
    }

    fn tagged_text(value: &str, top: f64, tag: &str) -> ParserChunk {
        let ParserChunk::Text(mut chunk) = text(value, top, 10.0) else {
            unreachable!()
        };
        chunk.pdfua_tag = Some(tag.into());
        ParserChunk::Text(chunk)
    }

    #[test]
    fn reconstructs_heading_paragraph_and_unordered_list() {
        let mut document = Document {
            file_name: "semantic.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page {
                index: 0,
                width: 300.0,
                height: 800.0,
                chunks: vec![
                    text("Title", 700.0, 18.0),
                    text("First paragraph line", 650.0, 10.0),
                    text("continues here", 635.0, 10.0),
                    text("- one", 580.0, 10.0),
                    text("- two", 565.0, 10.0),
                ],
            }],
            elements: Vec::new(),
        };
        reconstruct_semantics(&mut document);
        assert!(matches!(
            document.elements[0],
            SemanticElement::Heading { level: 3, .. }
        ));
        assert!(
            matches!(document.elements[1], SemanticElement::Paragraph { ref text, .. } if text == "First paragraph line continues here")
        );
        assert!(
            matches!(document.elements[2], SemanticElement::List { style: ListStyle::Unordered, ref items, .. } if items.len() == 2)
        );
    }

    #[test]
    fn keeps_long_large_font_body_text_as_a_paragraph() {
        let mut document = Document {
            file_name: "body.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page {
                index: 0,
                width: 300.0,
                height: 800.0,
                chunks: vec![text(
                    "A large body paragraph that is deliberately long enough to exceed the heading threshold and remain ordinary content.",
                    700.0,
                    28.0,
                )],
            }],
            elements: Vec::new(),
        };
        reconstruct_semantics(&mut document);
        assert!(matches!(document.elements[0], SemanticElement::Paragraph { .. }));
    }

    #[test]
    fn preserves_tagged_headings_captions_and_lists() {
        let mut document = Document {
            file_name: "tagged.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page {
                index: 0,
                width: 300.0,
                height: 800.0,
                chunks: vec![
                    tagged_text("Tagged heading", 700.0, "H2"),
                    tagged_text("Figure caption", 650.0, "Caption"),
                    tagged_text("first item", 600.0, "LI"),
                    tagged_text("second item", 580.0, "LI"),
                ],
            }],
            elements: Vec::new(),
        };
        reconstruct_semantics(&mut document);
        assert!(matches!(
            document.elements[0],
            SemanticElement::Heading { level: 2, .. }
        ));
        assert!(matches!(document.elements[1], SemanticElement::Caption { .. }));
        assert!(matches!(document.elements[2], SemanticElement::List { ref items, .. } if items.len() == 2));
    }

    #[test]
    fn keeps_tagged_structure_runs_as_separate_paragraphs() {
        let mut first = tagged_text("first", 700.0, "P");
        let mut second = tagged_text("second", 650.0, "P");
        if let ParserChunk::Text(chunk) = &mut first {
            chunk.structure_id = Some(1);
        }
        if let ParserChunk::Text(chunk) = &mut second {
            chunk.structure_id = Some(2);
        }
        let mut document = Document {
            file_name: "structure-runs.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page {
                index: 0,
                width: 300.0,
                height: 800.0,
                chunks: vec![first, second],
            }],
            elements: Vec::new(),
        };

        reconstruct_semantics(&mut document);

        assert_eq!(document.elements.len(), 2);
        assert!(matches!(document.elements[0], SemanticElement::Paragraph { ref text, .. } if text == "first"));
        assert!(matches!(document.elements[1], SemanticElement::Paragraph { ref text, .. } if text == "second"));
    }

    #[test]
    fn groups_same_line_tagged_runs_with_different_structure_ids() {
        let mut first = tagged_text("first", 700.0, "P");
        let mut second = tagged_text("second", 700.0, "P");
        if let ParserChunk::Text(chunk) = &mut first {
            chunk.structure_id = Some(1);
            chunk.bounds.right = 140.0;
        }
        if let ParserChunk::Text(chunk) = &mut second {
            chunk.structure_id = Some(2);
            chunk.bounds.left = 150.0;
        }
        let mut document = Document {
            file_name: "same-line-runs.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page {
                index: 0,
                width: 300.0,
                height: 800.0,
                chunks: vec![first, second],
            }],
            elements: Vec::new(),
        };

        reconstruct_semantics(&mut document);

        assert!(
            matches!(document.elements.as_slice(), [SemanticElement::Paragraph { text, .. }] if text == "firstsecond")
        );
    }

    #[test]
    fn reconstructs_border_table_rows_cells_and_headers() {
        let text = |value: &str, bounds: BoundingBox| {
            ParserChunk::Text(TextChunk {
                page_index: 0,
                bounds,
                text: value.into(),
                glyph_order: Vec::new(),
                character_spacing: None,
                font: FontInfo::default(),
                parser_order: 0,
                structure_id: None,
                pdfua_tag: None,
            })
        };
        let line = |bounds: BoundingBox| {
            ParserChunk::LineArt(LineArtChunk {
                page_index: 0,
                bounds,
                parser_order: 0,
            })
        };
        let mut chunks = Vec::new();
        for x in [10.0, 60.0, 110.0] {
            chunks.push(line(BoundingBox {
                left: x,
                bottom: 10.0,
                right: x,
                top: 100.0,
            }));
        }
        for y in [10.0, 55.0, 100.0] {
            chunks.push(line(BoundingBox {
                left: 10.0,
                bottom: y,
                right: 110.0,
                top: y,
            }));
        }
        chunks.extend([
            text(
                "Name",
                BoundingBox {
                    left: 15.0,
                    bottom: 80.0,
                    right: 45.0,
                    top: 90.0,
                },
            ),
            text(
                "Value",
                BoundingBox {
                    left: 65.0,
                    bottom: 80.0,
                    right: 95.0,
                    top: 90.0,
                },
            ),
            text(
                "A",
                BoundingBox {
                    left: 15.0,
                    bottom: 25.0,
                    right: 25.0,
                    top: 35.0,
                },
            ),
            text(
                "1",
                BoundingBox {
                    left: 65.0,
                    bottom: 25.0,
                    right: 75.0,
                    top: 35.0,
                },
            ),
        ]);
        let mut document = Document {
            file_name: "table.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: vec![Page {
                index: 0,
                width: 120.0,
                height: 120.0,
                chunks,
            }],
            elements: Vec::new(),
        };
        reconstruct_semantics(&mut document);
        let SemanticElement::Table { rows, .. } = &document.elements[0] else {
            panic!("expected a table element")
        };
        assert_eq!(rows.len(), 2);
        assert!(
            matches!(&rows[0], SemanticElement::TableRow { cells, .. } if matches!(&cells[0], SemanticElement::TableCell { is_header: true, children, .. } if matches!(&children[0], SemanticElement::Paragraph { text, .. } if text == "Name")))
        );
        assert!(
            matches!(&rows[1], SemanticElement::TableRow { cells, .. } if matches!(&cells[1], SemanticElement::TableCell { is_header: false, children, .. } if matches!(&children[0], SemanticElement::Paragraph { text, .. } if text == "1")))
        );
        assert_eq!(document.elements.len(), 1);
    }

    #[test]
    fn ignores_page_frame_lines_when_reconstructing_tables() {
        let page = Page {
            index: 0,
            width: 300.0,
            height: 400.0,
            chunks: vec![
                ParserChunk::LineArt(LineArtChunk {
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 0.0,
                        bottom: 399.0,
                        right: 300.0,
                        top: 400.0,
                    },
                    parser_order: 0,
                }),
                ParserChunk::LineArt(LineArtChunk {
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 0.0,
                        bottom: 0.0,
                        right: 1.0,
                        top: 400.0,
                    },
                    parser_order: 1,
                }),
                ParserChunk::Text(TextChunk {
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 20.0,
                        bottom: 100.0,
                        right: 80.0,
                        top: 120.0,
                    },
                    text: "body".into(),
                    glyph_order: Vec::new(),
                    character_spacing: None,
                    font: FontInfo::default(),
                    parser_order: 2,
                    structure_id: None,
                    pdfua_tag: None,
                }),
            ],
        };
        assert!(tables_from_page(&page).is_empty());
    }
}
