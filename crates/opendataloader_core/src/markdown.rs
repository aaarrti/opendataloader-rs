use crate::model::{Document, ListStyle, SemanticElement};

pub fn serialize_markdown(document: &Document) -> anyhow::Result<String> {
    let mut output = String::new();
    for element in &document.elements {
        if let Some(markdown) = render(element, 0) {
            output.push_str(&markdown);
            output.push_str("\n\n");
        }
    }
    Ok(output)
}

fn render(element: &SemanticElement, indent: usize) -> Option<String> {
    let text = |value: &str| markdown_text(value);
    match element {
        SemanticElement::Heading { level, text: value, .. } => {
            Some(format!("{} {}", "#".repeat((*level).clamp(1, 6) as usize), text(value)))
        }
        SemanticElement::Paragraph { text: value, .. }
        | SemanticElement::TextChunk { text: value, .. }
        | SemanticElement::TextBlock { text: value, .. }
        | SemanticElement::Caption { text: value, .. }
        | SemanticElement::TocItem { text: value, .. } => Some(text(value)),
        SemanticElement::Formula { content, .. } => Some(format!("$$\n{}\n$$", content)),
        SemanticElement::Image { reference, .. } => {
            reference
                .source
                .as_deref()
                .or(reference.data.as_deref())
                .map(|destination| {
                    let destination = if reference.data.is_some() && reference.source.is_none() {
                        destination.to_owned()
                    } else {
                        format_destination(destination)
                    };
                    format!("![{}]({destination})", "")
                })
        }
        SemanticElement::List { style, items, .. } => {
            let marker = match style {
                ListStyle::Ordered => "- ",
                ListStyle::Unordered => "- ",
            };
            let lines = items.iter().filter_map(|item| render_list_item(item, indent, marker));
            Some(lines.collect::<Vec<_>>().join("\n"))
        }
        SemanticElement::Table { rows, .. } => render_table(rows),
        SemanticElement::ListItem { .. }
        | SemanticElement::TableRow { .. }
        | SemanticElement::TableCell { .. }
        | SemanticElement::Toc { .. } => None,
    }
}

fn render_list_item(element: &SemanticElement, indent: usize, marker: &str) -> Option<String> {
    let SemanticElement::ListItem { children, text, .. } = element else {
        return render(element, indent);
    };
    let mut output = format!(
        "{}{}{}",
        "  ".repeat(indent),
        marker,
        text.as_deref().map(markdown_text).unwrap_or_default()
    );
    for child in children {
        if let SemanticElement::List { .. } = child
            && let Some(nested) = render(child, indent + 1)
        {
            output.push('\n');
            output.push_str(&nested);
        }
    }
    Some(output)
}

fn render_table(rows: &[SemanticElement]) -> Option<String> {
    let rows = rows.iter().filter_map(table_row).collect::<Vec<_>>();
    let first = rows.first()?;
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut output = String::new();
    output.push_str(&table_line(first, width));
    output.push('\n');
    output.push_str(&table_line(&vec!["---".to_owned(); width], width));
    for row in rows.iter().skip(1) {
        output.push('\n');
        output.push_str(&table_line(row, width));
    }
    Some(output)
}

fn table_row(element: &SemanticElement) -> Option<Vec<String>> {
    let SemanticElement::TableRow { cells, .. } = element else {
        return None;
    };
    Some(cells.iter().map(table_cell).collect())
}

fn table_cell(element: &SemanticElement) -> String {
    let SemanticElement::TableCell { children, .. } = element else {
        return String::new();
    };
    children.iter().filter_map(cell_text).collect::<Vec<_>>().join(" ")
}

fn cell_text(element: &SemanticElement) -> Option<String> {
    match element {
        SemanticElement::Heading { text, .. }
        | SemanticElement::Paragraph { text, .. }
        | SemanticElement::TextChunk { text, .. }
        | SemanticElement::TextBlock { text, .. }
        | SemanticElement::Caption { text, .. }
        | SemanticElement::TocItem { text, .. } => Some(markdown_text(text)),
        SemanticElement::Formula { content, .. } => Some(content.clone()),
        _ => None,
    }
}

fn table_line(values: &[String], width: usize) -> String {
    format!(
        "| {} |",
        (0..width)
            .map(|index| values.get(index).map_or("", String::as_str).replace('|', "\\|"))
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

fn markdown_text(value: &str) -> String {
    value
        .replace('\0', "")
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn format_destination(value: &str) -> String {
    let mut result = String::from("<");
    for character in value.chars() {
        match character {
            '<' | '>' | '\\' => {
                result.push('\\');
                result.push(character);
            }
            '\n' | '\r' => result.push(' '),
            _ => result.push(character),
        }
    }
    result.push('>');
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BoundingBox, DocumentMetadata, ElementCommon, FontInfo, ImageReference};

    fn common() -> ElementCommon {
        ElementCommon {
            id: Some(1),
            page_index: 0,
            bounds: BoundingBox {
                left: 0.0,
                bottom: 0.0,
                right: 1.0,
                top: 1.0,
            },
            pdfua_tag: None,
        }
    }

    fn document(elements: Vec<SemanticElement>) -> Document {
        Document {
            file_name: "test.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: Vec::new(),
            elements,
        }
    }

    #[test]
    fn renders_supported_elements_with_exact_separators() -> anyhow::Result<()> {
        let output = serialize_markdown(&document(vec![
            SemanticElement::Heading {
                common: common(),
                level: 8,
                text: "A & B".into(),
                font: FontInfo::default(),
            },
            SemanticElement::Paragraph {
                common: common(),
                text: "x<y>\0".into(),
                font: FontInfo::default(),
            },
            SemanticElement::Formula {
                common: common(),
                content: "x^2".into(),
            },
        ]))?;
        assert_eq!(output, "###### A &amp; B\n\nx&lt;y&gt;\n\n$$\nx^2\n$$\n\n");
        Ok(())
    }

    #[test]
    fn renders_lists_tables_and_sanitized_images() -> anyhow::Result<()> {
        let list = SemanticElement::List {
            common: common(),
            style: ListStyle::Ordered,
            items: vec![SemanticElement::ListItem {
                common: common(),
                text: Some("one".into()),
                children: vec![],
            }],
        };
        let table = SemanticElement::Table {
            common: common(),
            rows: vec![SemanticElement::TableRow {
                common: common(),
                row_number: 1,
                cells: vec![SemanticElement::TableCell {
                    common: common(),
                    row_number: 1,
                    column_number: 1,
                    row_span: 1,
                    column_span: 1,
                    is_header: true,
                    children: vec![SemanticElement::Paragraph {
                        common: common(),
                        text: "header".into(),
                        font: FontInfo::default(),
                    }],
                }],
            }],
        };
        let image = SemanticElement::Image {
            common: common(),
            reference: ImageReference {
                source: Some("images/a<b>\\c\n.png".into()),
                data: None,
                format: None,
            },
        };
        let output = serialize_markdown(&document(vec![list, table, image]))?;
        assert!(output.contains("- one\n\n"));
        assert!(output.contains("| header |\n| --- |\n\n"));
        assert!(output.contains("![](<images/a\\<b\\>\\\\c .png>)\n\n"));
        Ok(())
    }
}
