use crate::model::{BoundingBox, Document, ElementCommon, FontInfo, ListStyle, SemanticElement};
use serde_json::{Map, Value, json};

pub fn serialize_document(document: &Document) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(&document_value(document))?)
}

fn document_value(document: &Document) -> Value {
    json!({
        "file name": &document.file_name,
        "number of pages": document.page_count,
        "author": &document.metadata.author,
        "title": &document.metadata.title,
        "creation date": &document.metadata.creation_date,
        "modification date": &document.metadata.modification_date,
        "kids": document.elements.iter().map(element_value).collect::<Vec<_>>(),
    })
}

fn element_value(element: &SemanticElement) -> Value {
    match element {
        SemanticElement::Heading {
            common,
            level,
            text,
            font,
        } => text_value(common, "heading", Some("H"), font, text, |object| {
            object.insert("heading level".into(), (*level).into());
        }),
        SemanticElement::Paragraph { common, text, font } => {
            text_value(common, "paragraph", Some("P"), font, text, |_| {})
        }
        SemanticElement::TextChunk { common, text, font } => text_value(common, "text chunk", None, font, text, |_| {}),
        SemanticElement::TextBlock { common, text, font } => text_value(common, "text block", None, font, text, |_| {}),
        SemanticElement::Formula { common, content } => common_value(common, "formula", Some("Formula"), |object| {
            object.insert("content".into(), content.clone().into());
        }),
        SemanticElement::Image { common, reference } => common_value(common, "image", Some("Figure"), |object| {
            object.insert("alt_source".into(), "missing".into());
            if let Some(source) = &reference.source {
                object.insert("source".into(), source.clone().into());
            }
            if let Some(data) = &reference.data {
                object.insert("data".into(), data.clone().into());
                if let Some(format) = &reference.format {
                    object.insert("format".into(), format.clone().into());
                }
            }
        }),
        SemanticElement::Caption {
            common,
            text,
            linked_content_id,
        } => text_value(
            common,
            "caption",
            Some("Caption"),
            &FontInfo::default(),
            text,
            |object| {
                if let Some(id) = linked_content_id {
                    object.insert("linked content id".into(), (*id).into());
                }
            },
        ),
        SemanticElement::List { common, style, items } => common_value(common, "list", Some("L"), |object| {
            object.insert("numbering style".into(), list_style(style).into());
            object.insert("number of list items".into(), items.len().into());
            object.insert("list items".into(), items.iter().map(element_value).collect());
        }),
        SemanticElement::ListItem { common, children, text } => {
            common_value(common, "list item", Some("LI"), |object| {
                if let Some(text) = text {
                    object.insert("content".into(), text.clone().into());
                }
                object.insert("kids".into(), children.iter().map(element_value).collect());
            })
        }
        SemanticElement::Table { common, rows } => common_value(common, "table", Some("Table"), |object| {
            object.insert("number of rows".into(), rows.len().into());
            object.insert(
                "number of columns".into(),
                rows.iter()
                    .filter_map(|row| match row {
                        SemanticElement::TableRow { cells, .. } => Some(cells.len()),
                        _ => None,
                    })
                    .max()
                    .map_or(0, |columns| columns)
                    .into(),
            );
            object.insert("rows".into(), rows.iter().map(element_value).collect());
        }),
        SemanticElement::TableRow {
            common,
            row_number,
            cells,
        } => {
            let mut object = Map::new();
            object.insert("type".into(), "table row".into());
            if let Some(id) = common.id.filter(|id| *id != 0) {
                object.insert("id".into(), id.into());
            }
            object.insert("row number".into(), (*row_number).into());
            object.insert("cells".into(), cells.iter().map(element_value).collect());
            Value::Object(object)
        }
        SemanticElement::TableCell {
            common,
            row_number,
            column_number,
            row_span,
            column_span,
            is_header,
            children,
        } => common_value(
            common,
            "table cell",
            Some(if *is_header { "TH" } else { "TD" }),
            |object| {
                object.insert("row number".into(), (*row_number).into());
                object.insert("column number".into(), (*column_number).into());
                object.insert("row span".into(), (*row_span).into());
                object.insert("column span".into(), (*column_span).into());
                if *is_header {
                    object.insert("is_header".into(), true.into());
                }
                object.insert("kids".into(), children.iter().map(element_value).collect());
            },
        ),
        SemanticElement::Toc { common, items } => common_value(common, "toc", Some("TOC"), |object| {
            object.insert("toc items".into(), items.iter().map(element_value).collect());
        }),
        SemanticElement::TocItem { common, text, children } => {
            text_value(common, "toc item", Some("TOCI"), &FontInfo::default(), text, |object| {
                object.insert("kids".into(), children.iter().map(element_value).collect());
            })
        }
    }
}

fn common_value<F>(common: &ElementCommon, kind: &str, default_tag: Option<&str>, fill: F) -> Value
where
    F: FnOnce(&mut Map<String, Value>),
{
    let mut object = Map::new();
    object.insert("type".into(), kind.into());
    if let Some(tag) = common.pdfua_tag.as_deref().or(default_tag) {
        object.insert("pdfua_tag".into(), tag.into());
    }
    if let Some(id) = common.id.filter(|id| *id != 0) {
        object.insert("id".into(), id.into());
    }
    object.insert("page number".into(), (common.page_index + 1).into());
    object.insert("bounding box".into(), bounds_value(common.bounds));
    fill(&mut object);
    Value::Object(object)
}

fn text_value<F>(
    common: &ElementCommon,
    kind: &str,
    default_tag: Option<&str>,
    font: &FontInfo,
    text: &str,
    fill: F,
) -> Value
where
    F: FnOnce(&mut Map<String, Value>),
{
    common_value(common, kind, default_tag, |object| {
        object.insert("font".into(), font.name.clone().map_or(Value::Null, Value::String));
        object.insert("font size".into(), font.size.map_or(Value::Null, number));
        if let Some(color) = &font.color {
            object.insert("text color".into(), color.clone().into());
        }
        object.insert("content".into(), text.into());
        fill(object);
    })
}

fn bounds_value(bounds: BoundingBox) -> Value {
    Value::Array(
        [bounds.left, bounds.bottom, bounds.right, bounds.top]
            .into_iter()
            .map(number)
            .collect(),
    )
}

fn number(value: f64) -> Value {
    let rounded = (value * 1000.0).round() / 1000.0;
    serde_json::Number::from_f64(rounded).map_or(Value::Null, Value::Number)
}

fn list_style(style: &ListStyle) -> &'static str {
    match style {
        ListStyle::Ordered => "ordered",
        ListStyle::Unordered => "unordered",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BoundingBox, DocumentMetadata, ElementCommon, ImageReference};

    fn common() -> ElementCommon {
        ElementCommon {
            id: Some(1),
            page_index: 0,
            bounds: BoundingBox {
                left: 0.0,
                bottom: 0.0,
                right: 595.2759,
                top: 841.89,
            },
            pdfua_tag: None,
        }
    }

    #[test]
    fn serializes_root_nulls_and_image_fixture_shape() -> anyhow::Result<()> {
        let document = Document {
            file_name: "chinese_scan.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata {
                author: Some("anonymous".into()),
                title: Some("untitled".into()),
                ..Default::default()
            },
            pages: Vec::new(),
            elements: vec![SemanticElement::Image {
                common: common(),
                reference: ImageReference {
                    source: Some("images/imageFile1.png".into()),
                    data: None,
                    format: Some("png".into()),
                },
            }],
        };
        let value: Value = serde_json::from_str(&serialize_document(&document)?)?;
        assert_eq!(value["author"], "anonymous");
        assert!(value["creation date"].is_null());
        assert_eq!(value["kids"][0]["type"], "image");
        assert_eq!(value["kids"][0]["pdfua_tag"], "Figure");
        assert_eq!(value["kids"][0]["source"], "images/imageFile1.png");
        assert_eq!(value["kids"][0]["bounding box"][2], 595.276);
        Ok(())
    }

    #[test]
    fn serializes_nested_table_rows_and_cells() -> anyhow::Result<()> {
        let row = SemanticElement::TableRow {
            common: common(),
            row_number: 1,
            cells: vec![SemanticElement::TableCell {
                common: common(),
                row_number: 1,
                column_number: 1,
                row_span: 1,
                column_span: 1,
                is_header: true,
                children: Vec::new(),
            }],
        };
        let document = Document {
            file_name: "table.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata::default(),
            pages: Vec::new(),
            elements: vec![SemanticElement::Table {
                common: common(),
                rows: vec![row],
            }],
        };
        let value: Value = serde_json::from_str(&serialize_document(&document)?)?;
        assert_eq!(value["kids"][0]["number of columns"], 1);
        assert_eq!(value["kids"][0]["rows"][0]["cells"][0]["pdfua_tag"], "TH");
        assert_eq!(value["kids"][0]["rows"][0]["cells"][0]["is_header"], true);
        Ok(())
    }
}
