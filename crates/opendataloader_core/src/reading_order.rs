use crate::model::{Document, ElementCommon, SemanticElement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingOrder {
    Xycut,
    Off,
}

pub fn apply(document: &mut Document, mode: ReadingOrder) {
    if matches!(mode, ReadingOrder::Xycut) {
        document.elements.sort_by(|left, right| {
            let left_common = common(left);
            let right_common = common(right);
            left_common
                .page_index
                .cmp(&right_common.page_index)
                .then_with(|| right_common.bounds.top.total_cmp(&left_common.bounds.top))
                .then_with(|| left_common.bounds.left.total_cmp(&right_common.bounds.left))
        });
    }

    let mut next_id = 1;
    for element in &mut document.elements {
        assign_ids(element, &mut next_id);
    }
}

fn common(element: &SemanticElement) -> &ElementCommon {
    match element {
        SemanticElement::Heading { common, .. }
        | SemanticElement::Paragraph { common, .. }
        | SemanticElement::TextChunk { common, .. }
        | SemanticElement::TextBlock { common, .. }
        | SemanticElement::Formula { common, .. }
        | SemanticElement::Image { common, .. }
        | SemanticElement::Caption { common, .. }
        | SemanticElement::List { common, .. }
        | SemanticElement::ListItem { common, .. }
        | SemanticElement::Table { common, .. }
        | SemanticElement::TableRow { common, .. }
        | SemanticElement::TableCell { common, .. }
        | SemanticElement::Toc { common, .. }
        | SemanticElement::TocItem { common, .. } => common,
    }
}

fn assign_ids(element: &mut SemanticElement, next_id: &mut u64) {
    let common = match element {
        SemanticElement::Heading { common, .. }
        | SemanticElement::Paragraph { common, .. }
        | SemanticElement::TextChunk { common, .. }
        | SemanticElement::TextBlock { common, .. }
        | SemanticElement::Formula { common, .. }
        | SemanticElement::Image { common, .. }
        | SemanticElement::Caption { common, .. }
        | SemanticElement::List { common, .. }
        | SemanticElement::ListItem { common, .. }
        | SemanticElement::Table { common, .. }
        | SemanticElement::TableRow { common, .. }
        | SemanticElement::TableCell { common, .. }
        | SemanticElement::Toc { common, .. }
        | SemanticElement::TocItem { common, .. } => common,
    };
    common.id = Some(*next_id);
    *next_id += 1;

    match element {
        SemanticElement::List { items, .. } | SemanticElement::Toc { items, .. } => {
            for item in items {
                assign_ids(item, next_id);
            }
        }
        SemanticElement::ListItem { children, .. }
        | SemanticElement::TableCell { children, .. }
        | SemanticElement::TocItem { children, .. } => {
            for child in children {
                assign_ids(child, next_id);
            }
        }
        SemanticElement::Table { rows, .. } => {
            for row in rows {
                assign_ids(row, next_id);
            }
        }
        SemanticElement::TableRow { cells, .. } => {
            for cell in cells {
                assign_ids(cell, next_id);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{BoundingBox, DocumentMetadata, FontInfo};

    fn paragraph(page_index: usize, top: f64, left: f64, text: &str) -> SemanticElement {
        SemanticElement::Paragraph {
            common: ElementCommon {
                id: None,
                page_index,
                bounds: BoundingBox {
                    left,
                    bottom: top - 10.0,
                    right: left + 20.0,
                    top,
                },
                pdfua_tag: None,
            },
            text: text.into(),
            font: FontInfo::default(),
        }
    }

    fn document(elements: Vec<SemanticElement>) -> Document {
        Document {
            file_name: "test.pdf".into(),
            page_count: 2,
            metadata: DocumentMetadata::default(),
            pages: Vec::new(),
            elements,
        }
    }

    #[test]
    fn xycut_order_and_ids_are_deterministic() {
        let mut document = document(vec![
            paragraph(1, 700.0, 10.0, "second"),
            paragraph(0, 700.0, 10.0, "first"),
        ]);
        apply(&mut document, ReadingOrder::Xycut);
        assert_eq!(
            document
                .elements
                .iter()
                .map(|element| common(element).page_index)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
        assert_eq!(
            document
                .elements
                .iter()
                .map(|element| common(element).id)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
    }

    #[test]
    fn off_preserves_parser_order_and_assigns_ids() {
        let mut document = document(vec![
            paragraph(1, 700.0, 10.0, "second"),
            paragraph(0, 700.0, 10.0, "first"),
        ]);
        apply(&mut document, ReadingOrder::Off);
        assert_eq!(
            document
                .elements
                .iter()
                .map(|element| common(element).page_index)
                .collect::<Vec<_>>(),
            vec![1, 0]
        );
        assert!(document.elements.iter().all(|element| common(element).id.is_some()));
    }
}
