use std::{fmt, io, path::PathBuf};
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub left: f64,
    pub bottom: f64,
    pub right: f64,
    pub top: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DocumentMetadata {
    pub author: Option<String>,
    pub title: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FontInfo {
    pub name: Option<String>,
    pub size: Option<f64>,
    pub style: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextChunk {
    pub page_index: usize,
    pub bounds: BoundingBox,
    pub text: String,
    pub glyph_order: Vec<usize>,
    pub character_spacing: Option<f64>,
    pub font: FontInfo,
    pub parser_order: usize,
    pub structure_id: Option<u64>,
    pub pdfua_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImageChunk {
    pub page_index: usize,
    pub bounds: BoundingBox,
    pub object_reference: Option<String>,
    pub inline_image: Option<InlineImage>,
    pub parser_order: usize,
    pub structure_id: Option<u64>,
    pub pdfua_tag: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineImage {
    pub width: u32,
    pub height: u32,
    pub channels: u8,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LineArtChunk {
    pub page_index: usize,
    pub bounds: BoundingBox,
    pub parser_order: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParserChunk {
    Text(TextChunk),
    Image(ImageChunk),
    LineArt(LineArtChunk),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub index: usize,
    pub width: f64,
    pub height: f64,
    pub chunks: Vec<ParserChunk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReference {
    pub source: Option<String>,
    pub data: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementCommon {
    pub id: Option<u64>,
    pub page_index: usize,
    pub bounds: BoundingBox,
    pub pdfua_tag: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListStyle {
    Ordered,
    Unordered,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SemanticElement {
    Heading {
        common: ElementCommon,
        level: u8,
        text: String,
        font: FontInfo,
    },
    Paragraph {
        common: ElementCommon,
        text: String,
        font: FontInfo,
    },
    TextChunk {
        common: ElementCommon,
        text: String,
        font: FontInfo,
    },
    TextBlock {
        common: ElementCommon,
        text: String,
        font: FontInfo,
    },
    Formula {
        common: ElementCommon,
        content: String,
    },
    Image {
        common: ElementCommon,
        reference: ImageReference,
    },
    Caption {
        common: ElementCommon,
        text: String,
        linked_content_id: Option<u64>,
    },
    List {
        common: ElementCommon,
        style: ListStyle,
        items: Vec<SemanticElement>,
    },
    ListItem {
        common: ElementCommon,
        children: Vec<SemanticElement>,
        text: Option<String>,
    },
    Table {
        common: ElementCommon,
        rows: Vec<SemanticElement>,
    },
    TableRow {
        common: ElementCommon,
        row_number: usize,
        cells: Vec<SemanticElement>,
    },
    TableCell {
        common: ElementCommon,
        row_number: usize,
        column_number: usize,
        row_span: usize,
        column_span: usize,
        is_header: bool,
        children: Vec<SemanticElement>,
    },
    Toc {
        common: ElementCommon,
        items: Vec<SemanticElement>,
    },
    TocItem {
        common: ElementCommon,
        text: String,
        children: Vec<SemanticElement>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub file_name: String,
    pub page_count: usize,
    pub metadata: DocumentMetadata,
    pub pages: Vec<Page>,
    pub elements: Vec<SemanticElement>,
}

#[derive(Debug)]
pub enum ConversionError {
    InvalidInput { path: PathBuf, reason: String },
    PasswordProtected { path: PathBuf },
    Io { path: PathBuf, source: io::Error },
    Processing { path: PathBuf, reason: String },
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { path, reason } => write!(f, "invalid PDF input {}: {reason}", path.display()),
            Self::PasswordProtected { path } => write!(f, "password-protected PDF: {}", path.display()),
            Self::Io { path, source } => write!(f, "I/O error for {}: {source}", path.display()),
            Self::Processing { path, reason } => write!(f, "PDF processing error for {}: {reason}", path.display()),
        }
    }
}

impl std::error::Error for ConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_preserves_fixture_metadata_and_page_geometry() {
        let document = Document {
            file_name: "lorem.pdf".into(),
            page_count: 1,
            metadata: DocumentMetadata {
                author: Some("anonymous".into()),
                title: Some("untitled".into()),
                ..Default::default()
            },
            pages: vec![Page {
                index: 0,
                width: 595.276,
                height: 841.89,
                chunks: vec![ParserChunk::Text(TextChunk {
                    page_index: 0,
                    bounds: BoundingBox {
                        left: 72.0,
                        bottom: 766.698,
                        right: 378.11,
                        top: 783.358,
                    },
                    text: "Lorem".into(),
                    glyph_order: vec![0, 1, 2, 3, 4],
                    character_spacing: None,
                    font: FontInfo {
                        name: Some("Helvetica-Bold".into()),
                        size: Some(14.0),
                        ..Default::default()
                    },
                    parser_order: 0,
                    structure_id: None,
                    pdfua_tag: None,
                })],
            }],
            elements: Vec::new(),
        };

        assert_eq!(document.page_count, document.pages.len());
        assert_eq!(document.metadata.author.as_deref(), Some("anonymous"));
        assert_eq!(document.pages[0].index, 0);
        assert_eq!(document.pages[0].chunks.len(), 1);
    }

    #[test]
    fn model_represents_raster_image_reference() {
        let reference = ImageReference {
            source: Some("images/imageFile1.png".into()),
            data: None,
            format: Some("png".into()),
        };
        assert_eq!(reference.source.as_deref(), Some("images/imageFile1.png"));
        assert!(reference.data.is_none());
    }

    #[test]
    fn conversion_errors_remain_distinguishable() {
        let path = PathBuf::from("input.pdf");
        let invalid = ConversionError::InvalidInput {
            path: path.clone(),
            reason: "missing header".into(),
        };
        let password = ConversionError::PasswordProtected { path };
        assert!(invalid.to_string().contains("invalid PDF input"));
        assert!(password.to_string().contains("password-protected"));
    }
}
